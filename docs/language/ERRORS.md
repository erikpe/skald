# Errors and Exceptional Control Flow

Status: authoritative for current language-level compile-time rejection and
runtime-failure boundaries, the frozen panic design, implemented
primitive-operator failures, the frozen floating-point cast failure,
normal-flow cleanup obligations, and the maturity of recoverable exceptions.
The
[status matrix](STATUS.md) remains authoritative for compiler support.

## Compile-time rejection

A source file is accepted only when it satisfies the implemented grammar and
all name, type, access, initialization, lifetime, and entry-point rules. A
violated rule produces a compiler diagnostic and no executable program is
formed from that compilation.

Diagnostics identify the source problem and its relevant source location.
Recovery may allow the compiler to report additional independent problems in
the same file. The language contract does not promise a particular diagnostic
code, wording, number of follow-on diagnostics, or ordering between otherwise
independent errors.

Unsupported syntax is a compile-time error, not a reservation of that syntax
for future behavior. Likewise, a source shape accepted by the parser can still
be rejected by later semantic rules; grammar acceptance alone does not make a
program valid.

Tool usage, source I/O, target selection, backend legality, assembly, linkage,
and artifact-publication failures are compiler or toolchain failures rather
than source-language exceptions.

### Static-field declaration boundary

The implemented [zero-default static-field contract](STATIC_FIELDS.md) assigns
malformed shapes to syntax analysis and member collisions, inherited identity,
wrong-kind selection, non-callable use, and declaring-class privacy to
resolution. Type checking reports `TYP042` at a declaration whose type has no
complete all-zero live value. Every accepted primitive, inline-optional,
optional shared-owner, and inline-array declaration can be read or mutated
through its documented operations and lowers through typed static places to
verified always-live MIR roots.

Static storage adds no runtime failure or panic reason. Operations performed
through a static place retain their existing failures, such as optional
unwrap, array bounds, allocation, and ownership-count failure.

## Current runtime failures

The compiler implements executable `std::error::panic` call statements and
routes checked object casts, optional failures, array failures, checked shift
counts, checked integer zero divisors, and valid host-allocation exhaustion
through the common reporter below. Source shifts and verified checked-shift or
checked integer-division MIR route failures through the exact catalog reason.
Legal shared
and inline-backing ownership-count exhaustion uses that same reporter, while
invalid ownership state remains a hard compiler-defect trap. Skald has no
`throw`, `try`, `catch`, or other recoverable runtime-failure construct.

Foreign code reached through an external declaration may also fail to return
or may terminate the process. Such behavior lies beyond the guarantees of the
trusted [foreign-interoperation boundary](MODULES_AND_INTEROP.md#external-function-declarations).

The implemented
[checked-cast profile](POLYMORPHISM.md#checked-object-casts) has one
unrecoverable runtime failure. A failed dynamic cast terminates
unsuccessfully without returning to Skald or guaranteeing remaining
source-level cleanup. It does not introduce a catchable exception or settle a
general panic facility.

The implemented
[shared-ownership profile](SHARED_OWNERSHIP.md#unrecoverable-failures) applies
the same non-returning boundary to allocation failure and `u64` strong-count
overflow. Neither failure is catchable or guarantees remaining cleanup.
Its explicit `new T(copy source)` copy-allocation form completes any required
target-directed dynamic check before allocating its destination; failure
therefore cannot leave a copy destination awaiting cleanup.
Strong-count underflow, invalid handles, double finalization, and use after
release are compiler/runtime defects rather than source-level failures.

The implemented [object-cast profile](OBJECT_CASTS.md#failure) defines that a
dynamically unsuccessful cast
terminates without producing a null or invalid view and without guaranteeing
remaining cleanup. A statically impossible cast is rejected at compile time.
This applies to plain casts consumed directly and by owning inline copy
operations, owner-preserving shared casts, and target-directed shared copy
allocation.

## Frozen panic design

The frozen source API is:

```ska
from std::error import panic;

fn main() -> i64 {
    panic("configuration is missing");
}
```

The canonical declaration identity and signature are
`std::error::panic(message: std::str::Str) -> unit`. It is an ordinary public
module declaration: source must import it, import its module, or qualify it
through the normal module system. It is not in a prelude, and lower phases
must recognize its resolved intrinsic identity rather than the leaf name
`panic`. The declaration form and validation rules are owned by
[Modules and Foreign Interoperation](MODULES_AND_INTEROP.md#intrinsic-function-declarations).

The declaration grammar, canonical validation, ordinary import and
qualification behavior, identity, and execution are implemented. `TYP041`
is reserved for attempts to use panic in expression position.

A valid panic invocation is a call statement and does not return. It
satisfies definite return because its reachable path cannot reach the
enclosing block's closing brace. This design adds neither a general `never`
type nor arbitrary expression-position divergence; using the call where a
value is required is invalid.

The message is an ordinary exact-class value argument. The callee and its
message argument follow the normal receiver-before-arguments and
left-to-right evaluation rules, and the message is evaluated and
copy-constructed exactly once. If evaluating, copying, or otherwise producing
the message encounters an earlier unrecoverable failure, that earlier failure
wins and explicit panic reporting does not begin.

Once panic reporting begins, execution never returns to Skald. The process
terminates unsuccessfully and no remaining source-level cleanup is
guaranteed, including message-argument cleanup, later full-expression cleanup,
live locals, or owning value parameters. Panic has no handler edge, does not
unwind, and is not catchable. Future recoverable or checked exceptions must
retain their own values, edges, propagation, and cleanup rules; they cannot
reinterpret panic or the static failures below as exceptions.

Compiler-known source-reachable unrecoverable failures use the same reporter
as explicit panic, but retain distinct target-independent reasons through
verification. The runtime adds the reporter prefix and trailing LF; the
compiler or runtime supplies exactly the message bytes in this sole
authoritative catalog:

| Failure | Static message |
|---|---|
| Failed checked object cast | `checked object cast failed` |
| Absent optional access | `optional value is absent` |
| Optional presence-guard overflow | `optional presence guard overflow` |
| Guarded optional mutation | `cannot mutate a guarded optional value` |
| Invalid or overflowing array allocation request | `array allocation failed` |
| Array element index failure | `array index out of bounds` |
| Invalid array slice bounds | `array slice bounds are invalid` |
| Array slice length mismatch | `array slice length mismatch` |
| Shared or inline-backing ownership-count overflow | `ownership count overflow` |
| Host allocation failure for a valid request | `memory allocation failed` |
| Integer division by zero | `integer division by zero` |
| Integer remainder by zero | `integer remainder by zero` |
| Shift count at or above operand width | `shift count out of range` |
| Invalid `f64`-to-integer cast | `floating-point cast out of range` |

This catalog is closed for the frozen profile. A new compiler-known
source-reachable failure requires a deliberate language-contract revision, a
distinct semantic reason wherever target-independent verification needs one,
focused lowering coverage, and an end-to-end exact-stderr expectation.

Panic reporting is distinct from hard failure caused by a compiler or runtime
defect. Count underflow, null or otherwise invalid ownership handles, zero
live counts on a supposedly live allocation, missing or incompatible dynamic
metadata or finalizers, double finalization, impossible verified MIR states,
and violations of runtime ABI preconditions remain hard traps. They must be
prevented or eliminated as defects rather than converted into user-facing
panic records. The compiler/runtime hard-trap boundary is specified further
by the [backend contract](../compiler/BACKEND.md#panic-and-hard-trap-boundary)
and [runtime ABI](../compiler/RUNTIME_ABI.md#panic-reporting-abi).

The exact reporter signature, stderr bytes, and ABI-version transition are
implementation contracts rather than portable source representation. They
are frozen in the
[runtime ABI](../compiler/RUNTIME_ABI.md#panic-reporting-abi).
Source locations, a shadow trace stack, rendered stacktraces, trace-related
command-line policy, and exceptions are deliberately deferred.

## Optional failures

The [optional-values contract](OPTIONAL_VALUES.md#failure) defines three
unrecoverable failures:

- checked access to an absent optional;
- dynamic presence-guard count overflow; and
- clearing, replacing, or destroying an optional while a checked payload view
  keeps it present.

Checked access to an absent primitive, exact-class, or optional shared owning
value—including a local, field, parameter, or call result—is implemented and
lowers to the common non-returning reporter. Guard
overflow and guarded mutation are also executable for checked inline-class
payload views. Every failure occurs before producing an invalid payload or
changing guarded presence, does not return to Skald, and does not guarantee
remaining source-level cleanup. Each reason remains distinct through MIR and
selects its message from the [common panic reporter and catalog](#frozen-panic-design).

## Implemented operator failures

The implemented primitive operator profile includes three compiler-known,
source-reachable failures:

- integer `/` with a zero divisor;
- integer `%` with a zero divisor; and
- `<<` or `>>` with a `u64` count at or above the left operand's bit width.

Both operands evaluate exactly once from left to right before the check. A
failure uses its distinct target-independent reason and exact catalog message,
does not produce a value, never returns to Skald, and guarantees no remaining
source-level cleanup after reporting begins. A compiler must perform the
semantic check rather than expose a hardware division fault or masked shift
count.

`i64::MIN / -1` and `i64::MIN % -1` do not fail: they produce `i64::MIN` and
zero respectively. Integer wrapping overflow and floating division by zero
also do not use panic. The
[operator profile](TYPES_AND_VALUES.md#implemented-primitive-operator-profile)
defines their value behavior. Source checked shifts and integer division or
remainder use the common reporter through their verified failure edges.

## Frozen primitive cast failure

The frozen
[complete explicit primitive cast matrix](TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix)
adds one compiler-known source-reachable failure. Its distinct target-independent
reason, exact static message, and verified MIR failure edge are implemented;
x86-64 executes that failure edge through the common reporter.
An explicit `f64`-to-`i64`, `f64`-to-`u64`, or `f64`-to-`u8` cast evaluates its
source exactly once, truncates a finite value toward zero, and succeeds only
when that truncated mathematical integer is representable by the target. NaN,
infinity, and an out-of-range truncated result select the distinct
target-independent primitive-cast failure reason and exact catalog message
`floating-point cast out of range`.

Failure occurs before a result value exists, does not return to Skald, and
guarantees no remaining source-level cleanup after reporting begins. The
other twenty-two primitive cast pairs cannot fail. This boundary is not a
catchable exception, optional conversion result, target instruction fault, or
runtime-library conversion policy.

## Cleanup and abrupt termination

The implemented deterministic cleanup rules apply to normal block
fallthrough, conditional exits, and `return`. They are defined in
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#lifetime-registration-and-normal-cleanup).

If the process terminates inside the runtime or foreign code, execution does
not return to Skald and no remaining source-level cleanup is guaranteed. The
frozen panic design preserves this boundary explicitly. Abrupt unsuccessful
termination is distinct from recoverable exceptional control flow, which is
not implemented.

The frozen explicit array element-list contract follows this same boundary;
its primitive and exact-class slices are executable while optional,
nested-array, and shared-owner slices remain staged. Its outer backing remains
unpublished while one increasing
prefix contains live initialized elements. If allocation or an element
expression terminates unsuccessfully, current panic remains non-unwinding and
guarantees no prefix cleanup. Normal completion publishes only the fully
initialized array. Any future recoverable failure must destroy exactly the
completed prefix and release unpublished backing without treating later raw
slots as live.

Any future abrupt control flow that resumes or transfers within a Skald
program must extend the existing lifetime model rather than bypass it. At
minimum, its design must specify:

- exactly-once cleanup for every fully initialized owning value whose lifetime
  ends along the transfer;
- cleanup of initialized subobjects after failed construction or copying;
- that an incomplete complete object is not destroyed as though its lifetime
  had begun; and
- deterministic interaction with return destinations, temporaries, aliases,
  and nested scopes.

These are constraints on a future design, not implemented exceptional-cleanup
behavior.

## Standard I/O failures

The [standard I/O API](IO.md) is all-or-panic. Its standard-library
implementation translates private negative host results into stable
operation-specific messages for open, file read, stdin read, stdout write,
stderr write, and file close failures. It also uses `io: input too large` for
unrepresentable growth and `io: invalid runtime result` for an impossible
transfer count or non-progressing nonempty write.

Those messages carry neither paths nor host error numbers. They are ordinary
explicit panic messages selected by library code, not new compiler-known
termination reasons in the catalog above. Standard I/O adds no recoverable
value or exception edge, and a failed operation does not create a new cleanup
guarantee. The runtime byte operations and private `std::io` compiler
intrinsics, MIR/backend lowering, and all nine public functions are
implemented. Library code selects every stable failure above. This section
defines their integration with the existing
uncatchable panic policy.

## Recoverable and checked exceptions

Recoverable exceptions are an **exploratory direction**. No exception syntax,
exception type, throw set, handler, or unwinding rule is implemented or frozen.
Candidate words from older drafts are not reserved by the current grammar.

Before exceptions can become an implementation contract, a focused design
must settle:

- throw, rethrow, handler, and propagation source forms;
- whether exceptions are checked, unchecked, or divided into both categories;
- exception value types, ownership, access, and lifetime;
- callable, override, interface, and function-value compatibility;
- handler matching, ordering, binding scope, and unmatched propagation;
- failed construction, failed copying, destructor behavior, and nested failure;
- interaction with external functions and unrecoverable runtime termination;
  and
- deterministic cleanup on every exceptional edge, including ending optional
  presence guards before a recoverable transfer can cross a checked payload
  consumer.

Lowering through native unwinding, explicit control flow, hidden results, or
another mechanism is a compiler decision. No source rule should be inferred
from a possible implementation strategy. Shadow trace stacks, panic source
locations, and stacktrace rendering are also deferred, but are independent of
the exception model and may be implemented before it.

## Implementation boundary

Diagnostic codes, rendering format, compiler exit codes, phase recovery,
backend errors, and toolchain reporting belong to compiler documentation,
including [driver and artifacts](../compiler/DRIVER_AND_ARTIFACTS.md). Runtime
symbols, exact failure-reporting implementation, and the compatibility
contract belong to the
[runtime ABI](../compiler/RUNTIME_ABI.md).
