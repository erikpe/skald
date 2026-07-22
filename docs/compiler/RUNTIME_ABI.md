# Runtime ABI

Status: authoritative for the current compiler/runtime compatibility contract,
public C header, platform requirements, bootstrap output records, and runtime
responsibility boundary. Source-visible external declarations are owned by
[Modules and Foreign Interoperation](../language/MODULES_AND_INTEROP.md), and
target calling conventions are owned by the
[Backend and Target Contract](BACKEND.md).

## Artifact and public surface

The runtime is a C11 static library built as
`build/runtime/libskald_runtime.a`. Its public interface is
`runtime/include/skald_runtime.h`; declarations not present in that header are
implementation-private and are not ABI entry points.

The current public surface is:

```c
#include <stdbool.h>
#include <stdint.h>

#define SKALD_RUNTIME_ABI_VERSION UINT64_C(4)
#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v4

void SKALD_RUNTIME_ABI_MARKER(void);
uint64_t ska_rt_abi_version(void);

void ska_rt_println_i64(int64_t value);
void ska_rt_println_u64(uint64_t value);
void ska_rt_println_u8(uint8_t value);
void ska_rt_println_f64_bits(double value);
void ska_rt_println_bool(bool value);
```

These output functions are bootstrap observability facilities. They are not a
general formatting API, recoverable I/O API, or final standard-library design.
The compiler does not recognize their names specially: Skald programs declare
and call them through the ordinary restricted external-function mechanism.

## Version and link compatibility

ABI version 4 uses the exported no-op marker `ska_rt_abi_v4`. Every generated
process entry wrapper calls that exact symbol before entering Skald code. A
runtime archive with an older or otherwise incompatible marker therefore fails
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
- eight-bit bytes (`CHAR_BIT == 8`);
- IEC 60559 / IEEE-754 floating-point semantics;
- a binary C `double` with 64-bit storage, 53-bit significand, and the
  binary64 exponent range; and
- exact-width `int64_t`, `uint64_t`, and `uint8_t` types from `<stdint.h>`.

The implementation enforces the byte and binary64 properties at compile time.
The direct contract harness repeats them independently so a platform mismatch
fails while building the runtime suite. The current compiler target maps Skald
primitive values to these C types as described in the
[external C ABI](BACKEND.md#external-c-abi).

## Output records

Every successful output call writes one record to C `stdout` and flushes it
before returning. Records contain only the bytes specified below; there is no
locale-dependent formatting, carriage return, padding, grouping, or trailing
whitespace beyond the final line feed (`0x0a`). Consecutive calls produce
consecutive records in call order.

| Function | Successful record |
|---|---|
| `ska_rt_println_i64` | Shortest signed ASCII decimal spelling, including one leading `-` for negative values, then LF. The complete `int64_t` range is supported. |
| `ska_rt_println_u64` | Shortest unsigned ASCII decimal spelling of the complete `uint64_t` range, then LF. |
| `ska_rt_println_u8` | Shortest unsigned ASCII decimal spelling in `0` through `255`, then LF. |
| `ska_rt_println_bool` | Lowercase `true` or `false`, then LF. |
| `ska_rt_println_f64_bits` | Lowercase `0x`, exactly 16 lowercase hexadecimal digits for the received binary64 bits from most- to least-significant nibble, then LF. |

Integer zero is written as `0`; positive integers have no sign; and no integer
record has leading zeroes. Signed minimum is handled without signed overflow.

Floating output observes representation rather than formatting a decimal
number. It distinguishes positive and negative zero and preserves the received
bits of subnormals, infinities, and NaNs. It does not promise how compiler or
hardware arithmetic produced a NaN payload before the call.

## Detected output failure

Each output function requires a complete `fwrite` and, when that succeeds, a
successful following `fflush`. If either reports failure, the operation does
not return to Skald and the process terminates unsuccessfully. The ABI does not
promise diagnostic text, an exact exit status, or a particular terminating
signal. A failed write may already have made a partial record externally
visible.

The current implementation terminates through `_Exit(EXIT_FAILURE)`, avoiding
another implicit flush of the failed stream. That mechanism is private; the
stable contract is unsuccessful termination without a normal return.

## Responsibility boundary

The runtime currently owns only its version/link guard and the five output
operations above. It has no public ABI for:

- allocation or deallocation;
- shared ownership or reference counting;
- object, class, interface, or dynamic-type metadata;
- garbage collection, roots, tracing, safepoints, or write barriers;
- strings, arrays, files, or general I/O;
- panic reporting or runtime traces; or
- recoverable or checked exceptions.

Future language designs may require some of these responsibilities, but they
do not exist merely because a runtime library is present. A future addition
must first have a source-language contract, then define its runtime ownership,
failure behavior, ABI representation, version transition, and focused tests.

## Verification

`make runtime-test` builds the runtime and three directly linked C harnesses:

- the contract harness checks the marker, numeric version, and platform
  requirements;
- the output harness compares successful records byte for byte, including
  range boundaries and exact binary64 patterns; and
- the failure harness closes child-process stdout and requires every output
  function to terminate unsuccessfully.

[Driver tests](DRIVER_AND_ARTIFACTS.md#verification) prove that an archive
without the current marker fails linking without replacing an existing output
artifact. Native golden programs then exercise the same public symbols through
source declarations, backend call lowering, the real archive, and exact stdout
expectations.
