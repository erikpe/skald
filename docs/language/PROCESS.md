# Process Arguments

**Status:** implemented initial Linux profile.

This document defines the source-level contract for reading the invocation
vector of a Skald process. The installed implementation is ordinary Skald
standard-library source for the current Linux target. Availability is tracked
in [Language status](STATUS.md).

## Public surface

The standard library exposes exactly this initial process API:

```ska
public fn args() -> std::str::Str[];
```

It belongs to logical module `std::process` and is not part of the prelude.
Programs must import it through the ordinary module system, for example:

```ska
import std::process::args;

fn main() -> i64 {
    var invocation: std::str::Str[] = args();
    return (i64) invocation.len();
}
```

The only valid entry signature remains `fn main() -> i64`. Process arguments
are obtained by this library call, not by adding parameters to `main`.

## Invocation-vector and byte semantics

`args()` returns the complete host-supplied invocation vector in its original
order. If present, element zero is the invocation name supplied by the host. It
is not promised to be absolute, canonical, or the path of an existing file.
A host invocation with no vector entries produces an empty array.

Each element preserves its exact finite sequence of non-NUL bytes. Empty
arguments and arguments containing spaces, tabs, line feeds, or bytes that are
not valid UTF-8 remain distinct values. `Str` is a raw-byte value, so this API
does not perform shell tokenization, quoting, escaping, Unicode decoding,
locale conversion, or normalization.

## Snapshots and ownership

Each call reads and parses the host record anew and returns a fresh owning
`Str[]`. Calls do not share a mutable result array and do not use a static
cache or other process-global library state. Consequently, callers may replace
their own array elements without changing a result returned by another call.

The implementation counts records before allocating the exact-length
result array, then scans the same captured `Str` and assigns one checked
`Str.slice` for each record. Element strings may share that captured string's
backing storage. Ordinary synthesized ownership keeps the backing alive as
long as any returned slice needs it; backing identity and allocation count are
not source-observable.

The work is linear in the byte length of the host record plus its number of
arguments. Retained result storage is linear in those quantities. Reading the
record and constructing `Str` currently include the copies specified by
[Standard I/O](IO.md#byte-model); later implementations may remove
unobservable copies without changing this contract.

## Current Linux host contract

The implementation requires the Linux `/proc/self/cmdline` record and
reads it with `std::io::read_file`. That record is decoded as a sequence of
NUL-terminated arguments:

- every NUL completes one argument;
- consecutive NUL bytes therefore produce empty arguments;
- the required terminal NUL completes the final argument and does not create
  an additional one; and
- an empty record produces an empty array.

A nonempty record without its required final NUL violates the supported host
contract. No portable source behavior is promised for that malformed record,
and the implementation does not reinterpret its suffix or expose a generic
byte-splitting policy.

Failure to open, read, or close `/proc/self/cmdline` inherits the exact
all-or-panic behavior and messages of `std::io::read_file`. This API adds no
process-specific panic message, host error number, or recoverable error value.

Linux procfs is the required discovery mechanism for the current target, not a
portable language guarantee. A later target may replace only this library
mechanism while preserving the public invocation-vector, byte, snapshot, and
ownership semantics above.

## Language, compiler, and runtime boundary

The API composes existing declarations, calls, arrays, loops, primitive
operations and casts, strings, modules, and whole-file I/O. It adds no syntax,
grammar production, name-resolution rule, type-system rule, compiler phase,
IR operation, target instruction, or compiler intrinsic.

Runtime ABI version 8, `ska_rt_abi_v8`, the five byte-I/O functions, the public
C header, and the generated C-compatible process wrapper remain unchanged.
The wrapper continues to call the parameterless internal Skald entry function;
there is no C `argc`/`argv` capture, retained host pointer, runtime global, or
process-argument runtime API.

## Deliberate limits

This initial profile does not include environment variables,
current-directory accessors, executable-path canonicalization, argument
mutation, process spawning, exit APIs, signal handling, Windows command-line
reconstruction, non-Linux discovery, iterators, general collections, or a
general string-splitting API.
