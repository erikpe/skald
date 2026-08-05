# Standard I/O

**Status:** implemented whole-stream and primitive-line-output API.

This document defines the source-level contract for Skald's first standard I/O
module. The installed module exposes its public functions as ordinary Skald
code over five private byte-array intrinsics. The compiler resolves and types
intrinsic calls into dedicated I/O HIR and verified MIR, and x86-64 lowers
them to the narrow host byte operations provided by runtime ABI version 8.

The compiler and runtime contract behind this API is specified in
[Standard I/O compiler and runtime contract](../compiler/IO.md). Current
availability is summarized in [Language status](STATUS.md).

## Public surface

The standard library's explicitly imported `std::io` surface is:

```ska
public fn read_stdin() -> Str;
public fn read_file(ref path: Str) -> Str;
public fn write_stdout(ref text: Str) -> unit;
public fn write_stderr(ref text: Str) -> unit;
public fn println_bool(value: bool) -> unit;
public fn println_i64(value: i64) -> unit;
public fn println_u64(value: u64) -> unit;
public fn println_u8(value: u8) -> unit;
public fn println_f64(value: f64) -> unit;
```

These functions are ordinary Skald standard-library functions. Their private
intrinsics and host handles are implementation details: programs cannot import
or call them through this public surface. `std::io` is not part of the prelude.
All nine functions are currently available.

## Byte model

I/O preserves bytes exactly. `Str` remains Skald's raw byte-string type; these
functions do not validate, decode, normalize, or encode text. In particular,
their behavior is independent of UTF-8 validity.

This API does not add any `Str` conversion, borrowing, adoption, or builder
operation. Reads and exact writes use the existing copying conversions:

- `Str.to_bytes()` returns an independent `u8[]` copy.
- `Str.from_bytes(ref bytes: u8[])` copies the supplied bytes into a new `Str`.

Consequently, reads require storage for the accumulated byte array and the
resulting `Str`, and writes require a byte-array copy of the supplied `Str`.
Primitive line output additionally creates its canonical `Str` representation
through the existing `Str.from_<type>` methods. Those costs are part of this
initial design and may be optimized later without changing the public
signatures.

## Whole-input reads

`read_stdin()` blocks as required by the host and reads standard input until
end-of-file. It returns every byte in arrival order; empty input produces an
empty `Str`. It does not close standard input.

`read_file(path)` opens `path` for reading, reads until end-of-file, closes the
file after a successful read, and returns every byte in order. The initial host
implementation uses Linux path bytes. An embedded NUL byte is not a valid path.
No character decoding or newline transformation occurs.

Both reads start with a 64-byte array and grow geometrically as needed, then
copy the filled prefix into an exact-length array before `Str.from_bytes`
performs the final string copy. Their time and retained result space are linear
in the number of bytes read. Inputs that cannot be represented by a Skald array
fail rather than truncating.

## Exact output writes

`write_stdout(text)` writes every byte of `text` to standard output.
`write_stderr(text)` does the same for standard error. A successful return means
that the whole value was accepted by the underlying host writes, including when
several partial writes were required.

Neither function appends a newline, flushes C library buffers, nor closes the
standard handle. Writing an empty `Str` succeeds without requiring a host write.

Each `println_<type>(value)` converts its primitive with the correspondingly
named `Str.from_<type>` method, writes that canonical representation to
standard output, and then writes exactly one ASCII line-feed byte (`0x0A`).
The helpers add no spaces or other separators. `println_f64` uses the frozen
shortest round-tripping decimal contract; it is not a raw-bit observer and does
not preserve a NaN payload or sign.

## Failure behavior

The public API is all-or-panic. It does not expose host handles, error numbers,
partial progress, or recoverable error values. Failures use these stable
messages:

| Operation | Panic message |
|---|---|
| Open a file | `io: failed to open file` |
| Read an open file | `io: failed to read file` |
| Read standard input | `io: failed to read stdin` |
| Write standard output | `io: failed to write stdout` |
| Write standard error | `io: failed to write stderr` |
| Close a successfully read file | `io: failed to close file` |
| Grow beyond representable input | `io: input too large` |
| Observe an impossible runtime result | `io: invalid runtime result` |

Messages do not include the path or host error number. A panic aborts execution,
so this first API does not promise cleanup after an earlier operation fails.
`read_file` does close its handle on the normal success path before returning.

The frozen [process-argument contract](PROCESS.md) will reuse `read_file` to
read Linux `/proc/self/cmdline`. Process arguments remain a separate
`std::process` surface: they add no tenth public I/O function, private I/O
intrinsic, or runtime operation, and inherit these file-read failures exactly.

## Deliberate limits

This initial module does not include:

- format strings, interpolation, configurable numeric formatting, or parsing;
- public files, handles, streams, or incremental read/write APIs;
- file creation, writing, appending, seeking, metadata, or directory operations;
- line iteration, buffering controls, explicit flushing, asynchronous I/O, or
  non-blocking I/O;
- recoverable `Result` values, exceptions, or structured path values.

Those features require separate designs. They are not implied by the private
byte-array intrinsics used to implement this contract.
