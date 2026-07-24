# Errors and Exceptional Control Flow

Status: authoritative for current language-level compile-time rejection,
runtime-failure boundaries, normal-flow cleanup obligations, and the maturity
of recoverable exceptions. The [status matrix](STATUS.md) remains authoritative
for compiler support.

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

## Current runtime failures

The implemented source language has no `panic`, `throw`, `try`, `catch`, or
other runtime-failure construct. It also has no implemented bounds checks,
checked casts, allocation, optional extraction, integer division, or similar
operation requiring a general language panic policy.

The repository's bootstrap output functions are ordinary external calls. Their
current runtime contract terminates the process unsuccessfully when a write or
flush failure is detected. No exact diagnostic text, process status, or signal
is promised. This does not establish a general Skald panic mechanism.

Foreign code reached through an external declaration may also fail to return
or may terminate the process. Such behavior lies beyond the guarantees of the
trusted [foreign-interoperation boundary](MODULES_AND_INTEROP.md#external-function-declarations).

The implemented
[checked-cast profile](POLYMORPHISM.md#checked-object-casts) has one
unrecoverable runtime failure. A failed dynamic cast terminates
unsuccessfully without returning to Skald or guaranteeing remaining
source-level cleanup. It does not introduce a catchable exception or settle a
general panic facility.

The frozen, unimplemented
[shared-ownership design](SHARED_OWNERSHIP.md#unrecoverable-failures) applies
the same non-returning boundary to allocation failure and `u64` strong-count
overflow. Neither failure is catchable or guarantees remaining cleanup.
Its explicit `new T(copy source)` copy-allocation form completes any required
target-directed dynamic check before allocating its destination; failure
therefore cannot leave a copy destination awaiting cleanup.
Strong-count underflow, invalid handles, double finalization, and use after
release are compiler/runtime defects rather than source-level failures. These
rules do not change current compiler support or establish a general panic
facility.

The implemented [object-cast design](OBJECT_CASTS.md#failure) defines that a
dynamically unsuccessful cast
terminates without producing a null or invalid view and without guaranteeing
remaining cleanup. A statically impossible cast is rejected at compile time.
This applies to plain casts consumed directly and by owning inline copy
operations. Shared-owner casts remain unimplemented.

## Cleanup and abrupt termination

The implemented deterministic cleanup rules apply to normal block
fallthrough, conditional exits, and `return`. They are defined in
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#lifetime-registration-and-normal-cleanup).

If the process terminates inside the runtime or foreign code, execution does
not return to Skald and no remaining source-level cleanup is guaranteed. That
is distinct from recoverable exceptional control flow, which is not
implemented.

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
- deterministic cleanup on every exceptional edge.

Lowering through native unwinding, explicit control flow, hidden results, or
another mechanism is a compiler decision. No source rule should be inferred
from a possible implementation strategy.

## Implementation boundary

Diagnostic codes, rendering format, compiler exit codes, phase recovery,
backend errors, and toolchain reporting belong to compiler documentation,
including [driver and artifacts](../compiler/DRIVER_AND_ARTIFACTS.md). Runtime
symbols, exact output-failure implementation, and the
runtime compatibility contract belong to the
[runtime ABI](../compiler/RUNTIME_ABI.md).
