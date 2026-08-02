# Standard I/O Compiler and Runtime Contract

**Status:** runtime ABI, private compiler intrinsic pipeline, and public writes
implemented; public reads planned.

This document defines the compiler/runtime boundary for the source API in
[Standard I/O](../language/IO.md). Runtime ABI version 7 implements the five
host operations below. The closed compiler registry recognizes the five
private declarations installed in `std::io`; the module exposes its two public
writes while its two whole-input reads remain planned.

## Ownership boundary

The implementation is deliberately layered:

```text
public std::io functions written in Skald
    -> private canonical byte-array intrinsics
    -> dedicated HIR and MIR operations
    -> five narrow runtime ABI functions
    -> Linux host I/O
```

The standard library owns buffer growth, read-until-EOF loops, completion of
partial writes, conversion between `Str` and `u8[]`, and stable public panic
messages. The runtime owns only host handles, opening a byte path, one read or
write transfer, closing a handle, and translating host failures to negative
results.

## Canonical intrinsics

Only these private declarations are reserved for `std::io`:

```ska
intrinsic fn _io_standard_handle(stream: u8) -> i64;
intrinsic fn _io_open(ref path: u8[], mode: u8) -> i64;
intrinsic fn _io_read(handle: i64, mut ref destination: u8[], offset: u64) -> i64;
intrinsic fn _io_write(handle: i64, ref source: u8[], offset: u64) -> i64;
intrinsic fn _io_close(handle: i64) -> i64;
```

Their canonical identities are the fully qualified names under `std::io`, not
their short spelling. The compiler must reject matching declarations elsewhere,
wrong signatures, or direct use that does not resolve to the installed standard
module. The registry remains closed: spelling `intrinsic` does not create an
arbitrary external call.

The selectors are fixed:

- `_io_standard_handle`: `0` is stdin, `1` is stdout, and `2` is stderr.
- `_io_open`: mode `0` means open an existing path for reading.

No other selector or mode is valid in this ABI version.

## Result convention

Every intrinsic returns `i64` so success and failure have one uniform form:

- a non-negative standard-handle/open result is a host handle;
- a non-negative read/write result is the transferred byte count;
- read result `0` means end-of-file;
- close result `0` means success;
- any negative result is a private host failure.

The standard library branches only on the sign and translates failures to its
stable operation-specific panic messages. Host error numbers are intentionally
not part of the source contract. On Linux, the runtime represents an ordinary
system-call failure as the negative `errno` value.

An invalid selector, invalid mode, or invalid pointer/length pair is a compiler
or runtime contract defect, not a recoverable source-level condition.

## Array access, offsets, and lifetimes

The array parameters use Skald's existing whole-array alias rules:

- open and write borrow their arrays read-only;
- read borrows its destination mutably;
- an offset is valid only when `offset <= array.len()`;
- the compiler computes `data + offset` and `len - offset` before entering the
  runtime ABI;
- the normal array bounds panic path handles an invalid offset;
- the array anchor stays live for the duration of the host call.

Argument evaluation remains left-to-right and exactly once. Empty remaining
ranges use the language's established empty-array pointer convention; the
runtime must not dereference a pointer when its accompanying length is zero.

The runtime never receives a Skald array descriptor. Implemented x86-64
lowering extracts the data pointer and remaining length while retaining the
source-level alias and lifetime semantics in the compiler.

## Compiler phase contract

No grammar change is needed. Existing module declarations, intrinsic function
declarations, calls, arrays, aliases, loops, and conditionals are sufficient.

Resolution and type checking recognize the five canonical declarations,
enforce their exact signatures and access modes, and select dedicated I/O HIR
rather than generic direct calls. HIR contains semantic scalar values and
checked array aliases only; it contains no runtime symbol, target pointer, or
array layout.

MIR lowers those nodes to five dedicated semantic operations. Each operation
defines one exact `i64` value. Array operands retain an exact byte-array place,
required access, and a live backing anchor. Read and write materialize their
`u64` offset as an array range position and can reach the I/O operation only
through the successful `offset <= length` edge; equality therefore represents
an empty remaining range. The ordinary array index failure terminates the
larger-offset path. MIR contains no runtime symbol, descriptor layout, target
pointer, host `errno`, or implicit descriptor position.

Verification checks exact scalar and byte-array types, access compatibility,
live matching anchors (including an enclosing anchor for nested array aliases),
exact buffer-to-offset ownership, dominated successful bounds checks, balanced
storage lifetimes, unique initialized results, and the absence of residual
ordinary intrinsic calls. The x86-64 backend accepts this verified family and
selects only the corresponding `ska_rt_io_*` call. It materializes a null/zero
range for an empty descriptor without a header access, forms an end pointer for
`offset == length`, preserves call alignment and backing anchors, and stores
the returned signed `i64` in its ordinary value home.

The lowered offset check must occur before pointer arithmetic or the host call.
The returned count remains `i64` until generated standard-library code has
validated it. A count greater than the supplied remaining length, or zero
progress from a write with non-empty remaining input, is an invalid runtime
result and must not silently loop or truncate.

## Implemented runtime ABI version 7

The runtime exports these functions under ABI version 7:

```c
int64_t ska_rt_io_standard_handle(uint8_t stream);
int64_t ska_rt_io_open(const uint8_t* path, uint64_t path_length, uint8_t mode);
int64_t ska_rt_io_read(int64_t handle, uint8_t* destination, uint64_t capacity);
int64_t ska_rt_io_write(int64_t handle, const uint8_t* source, uint64_t length);
int64_t ska_rt_io_close(int64_t handle);
```

The ABI follows the existing Linux x86-64 System V calling convention. The
compiler owns Skald representation knowledge; the runtime sees only fixed-width
scalars and pointer/length pairs.

The runtime adapts the counted path to the host's terminated-path interface and
rejects embedded NUL bytes. An embedded-NUL or unrepresentable path produces an
appropriate negative host-style result. Read-only open requests close-on-exec.
Open, read, and write retry when interrupted before progress. A successful read
or write performs one host transfer and may return a partial count. A
zero-length transfer returns zero without dereferencing its pointer or entering
the host transfer. Larger transfers are capped to the host's representable
maximum. Close is attempted once and is not blindly retried after interruption
because the handle's state may already have changed.

The runtime does not allocate or grow Skald arrays, construct `Str`, loop to EOF,
complete partial writes, choose public panic text, append newlines, flush C
streams, or close the standard handles. Existing scalar observability helpers
remain a separate bootstrap surface in this ABI version.

## Verification obligations

Focused compiler tests cover all five HIR-to-MIR operations, deterministic MIR
forms, left-to-right single evaluation, byte-array aliases and backing-anchor
lifetimes, exact result carriage, malformed types/access/anchors/checks/results,
residual intrinsic calls, exact runtime-symbol selection, pointer/remaining-
length formation, empty ranges, assembler acceptance, and native version-7
archive linkage. Private replacement-standard-library goldens cover successful
results, host failures, dynamic offsets, and bounds failure before C.

Standard-library and native tests cover completed stdout/stderr writes, exact
binary bytes, forced partial transfers, invalid progress, and stable selected
panic messages. Remaining standard-library work must cover whole stdin and
file reads.

Direct C harnesses already cover runtime standard handles, open, close-on-exec,
empty and binary transfers, partial progress, EOF, negative host failures,
normal and repeated close, and hard contract defects independently of compiler
lowering.

Resolver and type-check tests already cover canonical identity,
exact-signature and private-access diagnostics, array-alias eligibility,
dedicated HIR, replacement providers, and deterministic resolved/HIR products.

The golden harness will need an explicit stdin fixture before stdin behavior can
be tested end to end. That harness capability is part of implementation, not a
change to the language contract.
