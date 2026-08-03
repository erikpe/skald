# Runtime ABI

Status: authoritative for the current compiler/runtime compatibility contract,
public C header, platform requirements, and runtime responsibility boundary.
Explicitly marked frozen additions confirm when a
selected future compiler feature leaves that boundary unchanged.
Source-visible external declarations are owned by
[Modules and Foreign Interoperation](../language/MODULES_AND_INTEROP.md), and
target calling conventions are owned by the
[Backend and Target Contract](BACKEND.md).

## Artifact and public surface

The runtime is a C11 static library built as
`build/runtime/libskald_runtime.a`. Its public interface is
`runtime/include/skald_runtime.h`; declarations not present in that header are
implementation-private and are not ABI entry points.

Optional values add no runtime entry point or ABI-version change. The
`shared? T` zero niche is handled entirely by generated branches; zero is
never passed to allocation, deallocation, finalization, or ordinary
shared-owner machinery.

The implemented
[zero-default static-field contract](../language/STATIC_FIELDS.md) likewise adds
no public symbol, startup or shutdown call, root-registration service, panic
reason, or ABI-version change. Static slots and their zero establishment are
compiler/backend-owned, and generated code intentionally performs no final
static cleanup. Runtime ABI version 8, `skald_runtime.h`, its compatibility
marker, and the generated process-entry wrapper remain unchanged. Static
fields do not alter instance object layouts, dispatch metadata, internal call
classification, or the primitive-only external ABI.

The current public surface is:

```c
#include <stdint.h>

#define SKALD_RUNTIME_ABI_VERSION UINT64_C(8)
#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v8

void SKALD_RUNTIME_ABI_MARKER(void);
uint64_t ska_rt_abi_version(void);

void *ska_rt_alloc(uint64_t byte_count);
void ska_rt_free(void *allocation);

_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length);

int64_t ska_rt_io_standard_handle(uint8_t stream);
int64_t ska_rt_io_open(const uint8_t* path, uint64_t path_length, uint8_t mode);
int64_t ska_rt_io_read(int64_t handle, uint8_t* destination, uint64_t capacity);
int64_t ska_rt_io_write(int64_t handle, const uint8_t* source, uint64_t length);
int64_t ska_rt_io_close(int64_t handle);
```

Primitive formatting, parsing, and line output are ordinary standard-library
code over `Str` and the byte-I/O boundary. The runtime has no primitive text
conversion or scalar observation entry point.

## Byte I/O ABI

ABI version 8 implements the five host operations over handles, scalars, and
byte pointer/length pairs shown in the public surface above. They form the
runtime half of the [standard I/O compiler and runtime contract](IO.md). The
compiler's canonical intrinsic identities, typed I/O HIR, verified
target-independent MIR, and x86-64 lowering are implemented. Generated calls
pass only fixed-width scalars and checked byte pointer/length pairs; array
descriptors and owners remain compiler-private.

`ska_rt_io_standard_handle` accepts selector `0` for stdin, `1` for stdout, or
`2` for stderr and returns the corresponding raw POSIX descriptor. Mode `0`
to `ska_rt_io_open` opens an existing raw byte path read-only and requests
close-on-exec. Open copies and terminates the counted path for the host call,
rejects embedded zero, frees the adaptation before returning, and retains no
path object.

Read and write perform at most one successful POSIX transfer. They retry
`EINTR` before progress, may return partial progress, and cap a request to
`SSIZE_MAX`. A zero-length transfer returns zero without dereferencing its
pointer or entering the host call. Close makes exactly one attempt and is not
blindly retried after `EINTR` because the descriptor state may already have
changed.

Non-negative open and standard-handle results are handles; non-negative read
and write results are transferred byte counts; read zero is EOF; and close
zero is success. An ordinary host failure is returned as negative `errno`.
Embedded-zero paths return `-EINVAL`, paths whose terminated representation
cannot be sized return `-ENAMETOOLONG`, and host allocation failure while
adapting a path returns `-ENOMEM`.

Invalid selectors, modes, negative or `int`-unrepresentable handles, and null
pointers paired with nonzero lengths are runtime contract defects and do not
return a host-style error. A nonnegative representable descriptor that the
host has closed or never assigned is structurally valid and ordinarily
returns `-EBADF` from read, write, or close.

This boundary exposes no `Str`, Skald array descriptor, shared owner, buffer
growth, whole-input loop, partial-write completion, public error message,
`FILE *` buffering, flushing, or standard-handle closure.

## Version and link compatibility

ABI version 8 uses the exported no-op marker `ska_rt_abi_v8`. Every generated
process entry wrapper calls that exact symbol before entering Skald code. A
version-7 or otherwise incompatible runtime archive therefore fails
normal linking with an undefined-symbol error rather than producing an
executable with mismatched compiler/runtime assumptions.

`ska_rt_abi_version()` returns `SKALD_RUNTIME_ABI_VERSION` for inspection and
direct runtime tests. It is not the compatibility guard because calling it
would already require a successful link.

An incompatible runtime change must update all of these together:

- the numeric version and version-specific marker in the public header;
- the marker exported by the runtime implementation;
- the marker referenced by every backend-generated process entry; and
- contract, link-mismatch, and native integration tests.

The marker name is deliberately version-specific. Keeping the old marker on an
incompatible runtime would defeat the link guard.

## C platform requirements

The current runtime implementation requires:

- C11 compilation;
- POSIX descriptors, `open`, `read`, `write`, and `close`, including
  close-on-exec support;
- the standard-error descriptor for allocation-free panic records;
- eight-bit bytes (`CHAR_BIT == 8`);
- exact-width `int64_t`, `uint64_t`, and `uint8_t` types from `<stdint.h>`.

The implementation enforces the byte property at compile time. The direct
contract harness repeats the runtime's platform requirements independently so
a mismatch fails while building the runtime suite. The current compiler target
maps Skald primitive values to these C types as described in the
[external C ABI](BACKEND.md#external-c-abi).

## Allocation and deallocation

`ska_rt_alloc(byte_count)` requires a nonzero byte count representable by C
`size_t`. It converts the count exactly, calls `malloc`, and returns suitably
aligned non-null storage of at least the requested size. A zero count or a
count that cannot be represented by `size_t` is a runtime contract defect and
uses the private hard-failure path. Host allocation failure for a valid
request calls `ska_rt_panic` with the catalog message `memory allocation
failed`. None of these paths returns.

`ska_rt_free(allocation)` requires the exact non-null base pointer returned by
one successful `ska_rt_alloc` call that has not already been freed. It passes
that pointer to `free` exactly once. Violating this precondition is a
compiler/runtime defect.

These functions know only byte counts and allocation base pointers. They do
not know object layout, initialize reference counts, inspect metadata, invoke
finalizers, retain owners, or release owners. `malloc`, `free`, and the common
unrecoverable termination helper remain implementation details.

## Panic-reporting ABI

ABI version 8 exports this reporting entry point:

```c
_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length);
```

`ska_rt_panic` receives a length-delimited byte sequence. `bytes` may be null
only when `length` is zero; otherwise it points to at least `length` readable
bytes. The runtime does not receive or inspect a Skald `Str`, array
descriptor, ownership header, class layout, or terminating zero. Generated
code extracts the backing byte address and logical length from the already
validated `std::str::Str` descriptor before crossing this ABI.

For a valid call, the reporter writes these bytes to C `stderr`, in order:

1. the seven ASCII bytes `panic: `;
2. exactly `length` bytes beginning at `bytes`; and
3. one line feed (`0x0a`).

Embedded zero and newline bytes are payload data and are written unchanged.
The reporter performs no allocation. It uses retrying direct writes to the
standard-error file descriptor, including partial-write and interruption
handling, so no C stream buffer or separate flush participates. After the
complete record, it terminates through `_Exit(EXIT_FAILURE)`. The language
guarantees unsuccessful termination but does not expose the platform's exact
numeric status. No Skald code resumes and no Skald cleanup runs after
reporting begins.

If writing the record fails, the reporter immediately calls
`_Exit(EXIT_FAILURE)`. A prefix or other partial record may already be
visible. The failure path does not call `ska_rt_panic` recursively and does not
attempt a second diagnostic.

A nonzero length with a null pointer, an unreadable range, or another violated
reporter precondition is a compiler/runtime defect. The runtime uses a private
hard-failure path for preconditions it can check; it must not render such a
defect as a Skald `panic:` record. The exact hard-failure instruction or
signal is private, but normal return is forbidden.

The source-level API, flow behavior, and sole static-message catalog are owned
by the [frozen language panic design](../language/ERRORS.md#frozen-panic-design).
Source locations, shadow trace state, stacktrace output, and exceptions are
not carried by this ABI and remain deferred.

## Responsibility boundary

The current version-8 runtime owns its version/link guard, checked byte
allocation/deallocation, panic reporter, and five narrow handle/byte I/O
operations above. It has no public ABI for:

- shared ownership or reference counting;
- object, class, interface, or dynamic-type metadata;
- garbage collection, roots, tracing, safepoints, or write barriers;
- strings, array descriptors, source-level files/streams, or broader I/O;
- runtime traces;
- recoverable or checked exceptions.

Future language designs may require more of these responsibilities, but they
do not exist merely because a runtime library is present.

The implemented
[array compiler contract](ARRAYS.md#internal-abi-and-runtime-boundary) requires
generated code to keep array length, element lifecycle, indexing, slicing,
backing anchors, shared counts, and finalization compiler-owned while reusing
the existing checked byte allocation and deallocation symbols. It therefore
adds no public C symbol or ABI-version change. Array construction and cleanup
exercise this boundary directly: nonempty inline arrays
use `ska_rt_alloc` and `ska_rt_free`, while empty arrays call neither. The
runtime remains unaware of descriptors, headers, lengths, element types, and
generated helper identities.

Shared ownership uses this minimal boundary as defined in the
[Shared-Ownership Compiler and Runtime Contract](SHARED_OWNERSHIP.md#minimal-c-runtime-abi).
Reference counting, metadata, anchors, and finalizer selection remain
compiler-owned.

The frozen
[optional-values compiler contract](OPTIONAL_VALUES.md#c-runtime-abi)
adds no public C symbol and requires no runtime ABI version change. Optional
state, presence guards, conditional lifecycle, and failure traps are
compiler-owned; an absent `shared? T` zero word never crosses into ordinary
shared-owner or allocator operations. Primitive and exact-class inline
optionals, including checked payload guard counts and failure traps, implement
this compiler-owned boundary without changing the runtime marker or adding a
runtime symbol.

The frozen [strings compiler contract](STRINGS.md) likewise adds no public C
symbol or ABI revision. Literal backing, array metadata relocations,
descriptor materialization, and immortal retain/release behavior are generated
compiler responsibilities; the runtime marker remains version 8.

Primitive integer comparisons likewise add no public C symbol or ABI revision.
The x86-64 backend selects signed or unsigned target conditions and
materializes canonical boolean results entirely in generated code; the runtime
marker remains version 8.

## Loop ABI boundary

The implemented `while`, `break`, and `continue`
[source contract](../language/FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits)
and [phase representation](PHASES_AND_IR.md#while-loop-representation)
add no public C symbol, runtime state, or ABI-version change. The runtime marker
remains `ska_rt_abi_v8`.

Loop execution is generated control flow. The runtime never receives or
interprets:

- source loop identities or labels;
- conditions, body blocks, latches, exits, or backedges;
- `break` or `continue` destinations;
- lexical scope or cleanup-depth metadata;
- MIR storage-lifetime epoch operations;
- loop-carried primitive values; or
- iteration counts or optimizer loop metadata.

Condition and body expressions may independently use existing runtime
allocation, deallocation, panic, or byte-I/O entry points under their ordinary
contracts. Deterministic destruction, shared retain/release, optional
state, array lifecycle, cleanup selection, and storage lifetime transitions
remain compiler-generated operations. Repetition does not create a runtime
loop service around those operations.

The loop feature adds no runtime-managed iterator, scheduler, safepoint, stack
growth, unwinding, tracing, or cancellation mechanism. A future iterator or
concurrency design may use ordinary Skald calls or may justify a separately
versioned runtime addition, but neither is implied by this loop contract.

Direct runtime ABI tests continue checking the unchanged header, symbols, and
version marker. Loop behavior, backward branches, and per-iteration cleanup
belong to compiler, backend, assembler, and native golden tests rather than a
new runtime harness.

## Implemented primitive operator ABI boundary

The
[implemented primitive operator profile](../language/TYPES_AND_VALUES.md#implemented-primitive-operator-profile)
adds no public C symbol, runtime-managed value, or ABI-version change. The
runtime marker remains `ska_rt_abi_v8`.

Wrapping arithmetic, division and remainder, bitwise operations, shifts,
floating operations and comparisons, boolean operations, short-circuit
control flow, result canonicalization, and path-dependent cleanup are
compiler-generated. The runtime never receives an operator identity, operand
type, shift count, floating status, logical branch, or temporary-lifetime
state.

Integer division by zero, integer remainder by zero, and excessive shift count
reuse the existing `_Noreturn ska_rt_panic(bytes, length)` entry point. The
compiler supplies their exact static message bytes from the sole
[language catalog](../language/ERRORS.md#frozen-panic-design); the reporter
does not classify the reason. These added callers do not change the reporter
signature, output record, termination behavior, or compatibility marker.

Operator verification belongs to compiler, backend, assembler, and native
tests. Direct runtime tests continue covering the generic reporter rather than
gaining operator-specific ABI harnesses.

Any future addition outside a separately frozen boundary must first have a
source-language contract, then define its runtime ownership, failure behavior,
ABI representation, version transition, and focused tests.

## Frozen complete primitive cast ABI boundary

The frozen
[complete explicit primitive cast matrix](../language/TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix)
adds no public C symbol, runtime-managed value, or ABI-version change. All
twenty-two pure cells are generated inline and retain the existing
`ska_rt_abi_v8` marker. The compiler catalogs the checked conversion's
distinct termination reason and exact static message while preserving that
marker. x86-64 executes the checked diamond and reaches the existing
reporter on failure without adding a runtime symbol or conversion helper.

Identity, integer, boolean, and integer-to-`f64` conversions are entirely
compiler-generated. Checked `f64`-to-integer conversion also remains
compiler-generated: verified MIR and target lowering own its finite and range
checks, truncation, result carriage, and failure edge. No `u64`-to-floating or
floating-to-integer conversion helper is part of the Skald runtime surface.

An invalid `f64`-to-integer cast reuses the existing
`_Noreturn ska_rt_panic(bytes, length)` entry point with the exact static bytes
`floating-point cast out of range`. The reporter receives neither operand nor
source/target type identity and performs no conversion or classification.
Adding this compiler-generated caller does not change the reporter signature,
output record, termination behavior, or compatibility marker.

Primitive-cast verification belongs to compiler, backend, assembler, and
native tests. Direct runtime tests continue covering the generic reporter and
unchanged symbol/version surface rather than gaining conversion-specific C
harnesses.

## Verification

`make runtime-test` explicitly depends on the runtime archive and then builds
six directly linked C harnesses:

- the contract harness checks the marker, numeric version, and platform
  requirements;
- the successful-allocation harness checks non-null suitably aligned writable
  storage across representative sizes and exact-base deallocation;
- the allocation-failure harness uses child processes to require unsuccessful
  termination for zero, host-unrepresentable sizes when applicable, and
  allocator failure;
- the successful I/O harness uses temporary files and pipes to check exact
  standard handles, close-on-exec read-only open, empty and binary transfers,
  partial progress, EOF, ordinary negative failures, normal close, and
  post-close failure;
- the I/O-defect harness uses child processes to keep invalid selectors,
  modes, handles, and pointer/length pairs on the private hard-failure path;
- the panic harness captures exact stderr for empty, ordinary, embedded-zero,
  and embedded-newline messages, verifies reporter-output failure, and keeps
  invalid reporter input on a silent private hard-failure path.

[Driver tests](DRIVER_AND_ARTIFACTS.md#verification) prove that a stale
version-7 archive fails the version-8 link guard without replacing an existing
output artifact. Native golden programs exercise public standard-I/O functions
over private intrinsic lowering through the real archive, including checked
ranges, ordinary negative host failures, and exact stdout expectations.
