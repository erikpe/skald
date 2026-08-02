# Standard I/O

**Status:** private compiler/runtime foundation implemented; public API planned.

This document defines the source-level contract for Skald's first standard I/O
module. The installed module currently contains only the five private
byte-array intrinsic declarations, and the compiler resolves and types their
calls into dedicated I/O HIR. Runtime ABI version 7 provides the narrow host
byte operations they will eventually target. The four public functions and
lowering are not implemented yet, so the existing scalar print helpers remain
the only source-callable bootstrap output facility.

The compiler and runtime contract behind this API is specified in
[Standard I/O compiler and runtime contract](../compiler/IO.md). Current
availability is summarized in [Language status](STATUS.md).

## Public surface

The standard library will provide an explicitly imported `std::io` module:

```ska
public fn read_stdin() -> Str;
public fn read_file(ref path: Str) -> Str;
public fn write_stdout(ref text: Str) -> unit;
public fn write_stderr(ref text: Str) -> unit;
```

These functions are ordinary Skald standard-library functions. Their private
intrinsics and host handles are implementation details: programs cannot import
or call them through this public surface. `std::io` is not part of the prelude.

## Byte model

I/O preserves bytes exactly. `Str` remains Skald's raw byte-string type; these
functions do not validate, decode, normalize, or encode text. In particular,
their behavior is independent of UTF-8 validity.

This API does not add any `Str` conversion, borrowing, adoption, or builder
operation. The implementation uses the existing copying conversions:

- `Str.to_bytes()` returns an independent `u8[]` copy.
- `Str.from_bytes(ref bytes: u8[])` copies the supplied bytes into a new `Str`.

Consequently, reads require storage for the accumulated byte array and the
resulting `Str`, and writes require a byte-array copy of the supplied `Str`.
Those costs are part of this initial design and may be optimized later without
changing the four public signatures.

## Whole-input reads

`read_stdin()` blocks as required by the host and reads standard input until
end-of-file. It returns every byte in arrival order; empty input produces an
empty `Str`. It does not close standard input.

`read_file(path)` opens `path` for reading, reads until end-of-file, closes the
file after a successful read, and returns every byte in order. The initial host
implementation uses Linux path bytes. An embedded NUL byte is not a valid path.
No character decoding or newline transformation occurs.

Both reads grow their internal buffer as needed. Their time and retained result
space are linear in the number of bytes read. Inputs that cannot be represented
by a Skald array fail rather than truncating.

## Exact output writes

`write_stdout(text)` writes every byte of `text` to standard output.
`write_stderr(text)` does the same for standard error. A successful return means
that the whole value was accepted by the underlying host writes, including when
several partial writes were required.

Neither function appends a newline, flushes C library buffers, nor closes the
standard handle. Writing an empty `Str` succeeds without requiring a host write.

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

## Deliberate limits

This initial module does not include:

- primitive formatting or parsing, or any new primitive-to-`Str` conversion;
- replacement or removal of the scalar observability print helpers;
- public files, handles, streams, or incremental read/write APIs;
- file creation, writing, appending, seeking, metadata, or directory operations;
- line iteration, buffering controls, explicit flushing, asynchronous I/O, or
  non-blocking I/O;
- recoverable `Result` values, exceptions, or structured path values.

Those features require separate designs. They are not implied by the private
byte-array intrinsics used to implement this contract.
