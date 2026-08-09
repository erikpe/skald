# Panic Runtime Trace Investigation

Status: completed and archived investigation against Skald commit
`49899bac3e6cedf79703d5a0656747f433b7c0fc` and Niflheim commit
`3dcd543620bfdc14c0b7c70a09364960e28174c9`. The frozen direction is recorded
in the
[panic runtime trace design record](PANIC_RUNTIME_TRACE_DESIGN_PROPOSAL.md),
which refines the linked native-frame design below by performing push and pop
through direct Linux x86-64 TLS access rather than per-activation C calls.

This investigation asks how Skald can attach source locations and a useful
call stack to its existing panic reporter while keeping successful execution
overhead small. It audits the sibling Niflheim implementation, identifies the
Skald phase and ABI boundaries that would change, compares implementation
choices, and recommends a design that improves on Niflheim's hot path without
changing panic into an exception.

## Executive conclusion

Skald already preserves almost all semantic information required for a panic
trace:

- every source belongs to a request-owned `SourceDatabase` with stable
  `SourceId`, byte-offset-to-line/column lookup, and a user-facing path;
- executable MIR definitions, instructions, basic blocks, and terminators
  retain `Span` values;
- ordinary, virtual, interface, lifecycle, and external calls retain their
  source spans;
- explicit panic and every compiler-known termination retain their exact
  source span through MIR; and
- MIR declarations retain module ownership and source names, so trace context
  names need not be reconstructed from mangled assembly symbols.

The missing pieces are a backend input that can resolve spans through the
source database, target-emitted static trace records, generated trace-frame
maintenance, and runtime rendering.

The recommended representation is one linked shadow frame embedded in each
source callable's existing native stack frame:

```c
typedef struct SkaRtTraceContext SkaRtTraceContext;
typedef struct SkaRtTraceLocation SkaRtTraceLocation;
typedef struct SkaRtTraceFrame SkaRtTraceFrame;

struct SkaRtTraceContext {
    const uint8_t* name;
    uint64_t name_length;
    const uint8_t* path;
    uint64_t path_length;
};

struct SkaRtTraceLocation {
    const SkaRtTraceContext* context;
    uint64_t line;
    uint64_t column;
};

struct SkaRtTraceFrame {
    SkaRtTraceFrame* previous;
    const SkaRtTraceLocation* location;
};

extern _Thread_local SkaRtTraceFrame* ska_rt_trace_top;
```

The runtime keeps only a thread-local pointer to the top frame. Generated code
pushes after establishing its native frame, pops on every normal return, and
loads/stores the TLS top directly through the local-exec model. Changing
location loads one static `SkaRtTraceLocation` address and stores it directly
into the current frame's `location` field. Panic walks the still-live linked
frames from newest to oldest.

This gives the desired push/replace/pop behavior with no heap allocation, no
capacity checks, per-activation C calls, reserved general register, or new
failure caused by trace bookkeeping. On Linux x86-64 the proposed sequences
are six instructions for push, two for pop, and two for replacement. Compared
with Niflheim, dynamic trace storage falls from a heap-resident 24-byte record
per activation to a 16-byte record in the native frame that already bounds
the activation's lifetime.

## Current Skald boundary

The implemented panic contract prints exactly one length-delimited record:

```text
panic: <message bytes>\n
```

`ska_rt_panic` performs allocation-free direct writes and exits through
`_Exit`. Explicit `std::error::panic`, MIR `Terminate` reasons, valid host
allocation failure, and legal ownership-count overflow all reach that same
reporter. Invalid verified state remains a silent hard trap. Panic has no
unwind edge and performs no remaining Skald cleanup.

The existing contracts deliberately defer locations and traces in:

- [Errors and Exceptional Control Flow](../language/ERRORS.md#frozen-panic-design);
- [Compiler Phases and IR](../compiler/PHASES_AND_IR.md#frozen-panic-and-termination-representation);
- [Backend and Target Contract](../compiler/BACKEND.md#panic-and-hard-trap-boundary); and
- [Runtime ABI](../compiler/RUNTIME_ABI.md#panic-reporting-abi).

No source grammar change is needed. This is compiler, target, runtime, driver,
and observable-output work; it remains independent of recoverable exceptions
and exceptional cleanup.

### Existing source information

`source::Span` is a `SourceId` plus a half-open UTF-8 byte range.
`SourceFile::location` converts a byte offset to one-based line and Unicode
scalar column. The driver still owns the complete `SourceDatabase` when it
calls backend emission, but the current backend facade accepts only
`&MirProgram`. The first required boundary change is therefore to give
backend emission explicit, request-owned trace/source metadata rather than
copying source text into MIR or introducing a global source registry.

`MirProgram::modules` also maps every reachable module to its source identity,
logical module path, provider-relative path, and display path. That makes a
stable root-relative trace path possible without Niflheim's after-the-fact
common-prefix calculation.

### Existing executable spans

MIR is already sufficiently annotated for precise trace sites:

- `MirDefinitionRef::span()` identifies callable entry;
- every `MirInstruction` exposes `span()`;
- every `MirTerminator` exposes `span()`;
- `MirCall`, initialization, copying, assignment, and cleanup retain the
  source operation that caused a nested callable to run; and
- `MirTerminator::Panic` and `MirTerminator::Terminate` retain the source
  failure site.

Generated array, ownership, finalization, static-lifecycle coordinator, and
entry-wrapper helpers do not need to become user-visible frames. Their caller
sets its own location before entering a helper; a helper-entered user body,
such as a destructor, then pushes its ordinary source context.

## Niflheim audit

### Audit basis

The audit inspected these Niflheim owners at the commit named above:

- `compiler/backend/targets/x86_64_sysv/trace_codegen.py` and the matching
  AArch64 module;
- both targets' `emit.py` callable, call-site, safepoint, constructor, and
  epilogue paths;
- `compiler/backend/targets/api.py` and `compiler/cli.py` option plumbing;
- `runtime/include/runtime.h`, `runtime/src/runtime.c`, and
  `runtime/src/panic.c`;
- `docs/ABI_NOTES.md`; and
- backend and CLI trace tests.

Eight focused backend and CLI tests passed during this investigation. Search
of the runtime and integration suites found assembly-hook and option coverage,
but no direct runtime trace-stack harness and no exact complete stacktrace
expectation. Panic integration tests check that the panic-message substring is
present, so trace formatting and frame correctness have weaker regression
protection than the hook emission.

### Implemented Niflheim model

Niflheim enables runtime tracing by default and exposes
`--omit-runtime-trace`. For each emitted source callable it creates two
NUL-terminated static strings: a formatted callable name and a normalized
file path. Its runtime frame is:

```c
struct RtTraceFrame {
    const char* function_name;
    const char* file_path;
    uint32_t line;
    uint32_t column;
};
```

Generated code calls:

- `rt_trace_push(function_name, file_path, line, column)` in the callable
  prologue;
- `rt_trace_set_location(line, column)` before ordinary calls and selected
  runtime/safepoint operations; and
- `rt_trace_pop()` in the common epilogue, with return registers preserved
  around the call.

Call-site updates happen before direct, member, and indirect calls. This is an
important correctness property: while a callee is active, the caller's frame
describes the call expression that led to it rather than an older operation.
Constructor wrapper and implementation emission avoid pushing the same source
constructor twice. On both native targets, GC root publication occurs before
the trace push.

The runtime stores frames in a process-global growable array inside
`RtThreadState`. Capacity starts at eight and doubles through `realloc`.
`rt_trace_set_location` mutates the top frame, `rt_trace_pop` decrements the
size, and panic prints:

```text
panic: <message>
location: <top-file>:<line>:<column>
stacktrace:
  at <top-function> (<top-file>:<line>:<column>)
  ...
```

The stack is printed newest first and the top location is intentionally
duplicated in the separate `location:` line.

### Lessons to retain

Niflheim demonstrates several sound choices:

- trace state is independent of GC roots and source-level cleanup;
- callable entry, caller call sites, and normal exit are separate hooks;
- the active frame's location changes without pushing artificial frames;
- generated code refers to static metadata rather than copying source names
  dynamically;
- tracing can be omitted at compilation; and
- x86-64 and AArch64 expose the same target-independent behavior behind
  target-specific instruction emission.

### Costs and gaps not to copy

The exact implementation has avoidable costs for Skald:

- every location change crosses the C ABI, even though it only replaces two
  integers in the top frame;
- every push passes four arguments and copies 24 bytes into a separate heap
  array;
- the first and occasional later pushes can allocate;
- trace allocation failure itself calls panic, so enabling diagnostic
  bookkeeping can change whether an otherwise valid program completes;
- trace state is process-global rather than thread-local;
- pop underflow is rendered as a user-looking panic rather than remaining a
  runtime/compiler defect;
- `fprintf` and `abort` do not preserve Skald's allocation-free, exact-byte,
  immediate-exit reporter properties;
- names and paths are terminating strings rather than length-delimited data;
  and
- exact runtime rendering and nested frame behavior lack focused tests.

These are not reasons to discard the shadow-stack model. They point toward a
smaller record, native-frame-owned storage, direct replacement, length-
delimited metadata, allocation-free rendering, and stronger tests.

## Compared implementation choices

| Choice | Successful-execution cost | Panic-time cost | Main problems |
|---|---|---|---|
| Niflheim-style runtime vector | Three runtime hook families; 24-byte copy per push; possible `realloc` | Direct reverse array walk | Allocation can change semantics; every update is a call; global state |
| Pointer-only runtime vector | One pointer copied per push/update; possible growth | Indirect static-record reads | Still allocates or needs truncation/capacity policy |
| Linked frame in each native stack frame | Enter/leave per source callable; 16 frame bytes; location replacement is address load plus store | Linked walk through live native frames | Requires a small trace-frame ABI and frame-layout integration |
| Native frame-pointer walk plus PC mapping | Near-zero explicit hot-path work | Unwind and symbolize only on panic | Requires reliable unwind metadata/symbolization, complicates foreign frames and deterministic self-contained output |
| DWARF/libunwind stacktrace | Debug metadata only during success | General unwinding and symbolization on panic | New system dependencies, larger complexity, less deterministic deployment |

Native unwinding is attractive if zero successful-path instrumentation is the
only objective, but it is disproportionate for the current one-target,
self-contained runtime. Skald already uses fixed native frames, and panic is
non-unwinding, so a linked shadow frame has a much smaller correctness and
portability surface. It also realizes the requested push/replace/pop model
directly.

## Recommended Skald design

### Static metadata

Emit three interned target-private pools when tracing is enabled:

1. one byte blob for each used context name and source path;
2. one `SkaRtTraceContext` for each source callable; and
3. one `SkaRtTraceLocation` for each distinct callable and span-start
   location that generated code uses.

The trace stack stores only a location-record address. The location points to
its callable context, and the context points to length-delimited name and path
bytes. On x86-64, a location record is 24 bytes and a context record is 32
bytes. This static size is paid per distinct emitted site, not per execution.
Records containing relocations belong beside existing deterministic
relocation-read-only metadata. Stable pool order must follow deterministic
semantic identity and location order, never hash iteration or host addresses.

Context names should be derived from `ProgramModuleTable` and MIR declaration
names, not mangled symbols. A useful family is:

```text
app::main
app::Widget.make
app::Widget.init#1
app::Widget.copy
app::Widget.assign
app::Widget.destroy
app::Widget.value::<static-init>
```

Initializer overloads need an ordinal or signature discriminator. Generated
entry, array, ownership, finalization, and coordinator helpers should not
appear as source frames.

### Runtime frame ownership

Tracing adds one 16-byte `SkaRtTraceFrame` allocation to the fixed frame layout
of every source callable. The prologue must:

1. establish and reserve the native frame;
2. preserve incoming parameters through the ordinary prologue;
3. initialize/push the trace frame with direct local-exec TLS operations; and
4. begin body instruction selection.

The runtime defines one hidden `_Thread_local` top pointer. Generated code
loads it into a transient caller-saved scratch register, writes `previous` and
the initial `location`, and publishes the frame address back through TLS. It
performs no C call or allocation and permanently reserves no register.

Every ordinary return path restores the TLS top directly from `previous`
before loading its final scalar, floating, shared-owner, or hidden-result
return value from its frame home. No pop occurs on panic, because the live
frames are exactly what the reporter must inspect.

### Location replacement policy

Generated x86-64 location replacement should be only:

```text
lea rax, [rip + <location-record>]
mov qword ptr [rbp + <trace-location-home>], rax
```

The compiler should update at observable failure/call boundaries, not before
every MIR instruction:

- before every source-visible internal, virtual, interface, lifecycle, or
  external call, so the caller frame records the active call site;
- before entering a generated helper on behalf of a source operation, while
  leaving the helper itself absent from the trace;
- immediately before a runtime operation that can report panic internally,
  currently `ska_rt_alloc`;
- on the failure edge immediately before an explicit or compiler-known panic
  reporter call, keeping the successful checked path free of that update; and
- before any future runtime call whose contract permits it to invoke the
  common reporter.

Pure arithmetic, stores, cleanup bookkeeping, unconditional branches, and
hard-trap-only checks need no update because panic cannot observe those
locations. Backend lowering should carry the current source span while
selecting nested helper calls so an implicit lifecycle or ownership call uses
the originating MIR operation's location.

A small target-private dataflow/coalescing pass may omit a replacement when
all incoming paths already establish the same location record. This matters
for loops that repeatedly reach the same call site. It is an optimization,
not a prerequisite for correct first implementation.

### Compiler ownership

The clean boundary is an explicit backend input/options object containing:

- the verified final `MirProgram`;
- read-only access to the request's `SourceDatabase` when trace emission is
  enabled; and
- trace-emission policy.

The backend resolves `Span` starts to line and column exactly once while
building target metadata. MIR should continue to retain semantic spans, not
rendered paths or target record IDs. The runtime should know only the public
trace-record ABI, not `SourceId`, byte offsets, module identities, Skald
strings, or MIR.

`backend::emit_assembly` currently accepts only `&MirProgram`, while the
driver still has `SourceDatabase` immediately before that call. Plumbing the
database through an explicit backend request preserves request ownership and
the architecture's forward dependency rule. Hand-built MIR tests can disable
trace emission unless they deliberately supply a source database.

The x86-64 machine model needs explicit trace-enter, trace-location, and
trace-leave sequences or sufficiently typed primitive instructions plus
trace metadata in `AssemblyProgram`. Keeping trace record construction and
emission in a cohesive target module avoids scattering record layout across
ordinary call, termination, frame, and assembly emitters.

### Path policy

Trace paths must be useful without making output depend on temporary build
roots. Prefer the module provenance `root_relative_path` for loaded modules;
it is deterministic, relocatable, and already separated from canonical I/O
identity. Positional outside-root entries can retain their configured display
spelling when relative, or an explicitly documented fallback when absolute.

This policy needs confirmation because it differs from diagnostic rendering,
which currently uses `SourceFile::path().display()`. It is nevertheless safer
than Niflheim's common-prefix stripping and prevents golden traces from
embedding per-run temporary directories. Trace paths should be rendered or
escaped into one line at compile time and retained as length-delimited bytes;
raw newline or control bytes in a host path must not forge trace rows.

### Reporter output

Keep the existing reporter signature:

```c
_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length);
```

The runtime can inspect its own thread-local top after writing the existing
message. This is important for allocation failure: `ska_rt_alloc` can keep
calling the same reporter without receiving a trace argument. A recommended
non-duplicating shape is:

```text
panic: configuration is missing
stacktrace:
  at app::load (app/load.ska:18:9)
  at app::main (app/main.ska:7:5)
```

When no frame exists, preserve the current one-line record. Do not add
Niflheim's separate `location:` line because the first stack row already owns
that information.

Trace rendering must reuse direct retrying descriptor writes, perform no
allocation, convert line and column through fixed local buffers, and stop
immediately if any write fails. Hard traps remain silent. Context names and
paths are compiler-controlled length-delimited fields; the panic message
remains arbitrary raw bytes and may contain embedded zero or newline exactly
as today.

The new structs and hidden TLS symbol are an incompatible compiler/runtime ABI
addition, so the numeric ABI and link marker must advance together. The panic
function's own C signature need not change.

### Enablement and zero-cost omission

Follow Niflheim in making trace emission a compilation choice. When omitted,
the compiler must emit:

- no trace frame homes;
- no TLS push/pop instructions;
- no location stores; and
- no context, path, or location records.

This gives an actual zero-overhead mode rather than a runtime switch that
continues maintaining an unused stack. Whether tracing is enabled by default
is still a product decision. Default-on maximizes panic usefulness and matches
Niflheim; default-off preserves current code size, speed, and exact stderr.
The decision should be based on focused measurements and the intended role of
release builds, not on implementation convenience.

## Expected performance profile

With the recommended enabled design:

- per source activation: 16 additional fixed-frame bytes, six inline push
  instructions, and two inline pop instructions;
- per relevant location change: one RIP-relative address load and one stack
  store, with no call and no allocation;
- per generated helper activation: no frame unless it represents an actual
  source callable;
- per failure-only static check: no successful-path location store when the
  store is placed in the failure block; and
- per executable: static context/location records and their strings only for
  emitted, used trace sites.

The unavoidable successful-path updates are source call sites and runtime
operations such as allocation that may themselves panic. A benchmark should
separate call-heavy recursion, tight loops with no observable failure point,
allocation-heavy code, and ordinary application goldens. Assembly tests
should also count push/pop and location stores so a later refactor cannot
silently reintroduce a function call per location as in Niflheim.

## Correctness cases

The implementation must settle and test these cases explicitly:

- direct, recursive, virtual, interface, static, initializer, copy,
  assignment, and destructor call chains;
- caller call-site location while a callee panics;
- panic during caller-side argument production or copying before callee entry;
- explicit dynamic-message panic after message production;
- every MIR termination reason, including failure-only location updates;
- host allocation failure inside `ska_rt_alloc`;
- ownership overflow emitted inside ordinary lowering and generated helpers;
- static initialization and destruction, while keeping synthetic coordinator
  frames out of output;
- normal returns of unit, integers, byte/bool, floating, shared, optional
  shared, and caller-destination object results after trace leave;
- abrupt panic without trace pops or cleanup;
- empty trace state for direct C runtime callers;
- compile-time trace omission with no residual symbols, frame bytes, or
  instructions;
- deterministic names, paths, metadata order, assembly, and exact stderr
  across separate processes and temporary roots; and
- output failure partway through the trace without recursion or buffered I/O.

Runtime C tests should directly build nested frames, mutate a top frame's
location exactly as generated code does, and verify newest-first output.
Backend tests should own exact TLS relocation, hook placement, linked-pop, and
ABI-preserving return order. Native goldens should own complete source-to-
stderr call chains. Existing plain panic expectations can remain useful under
the omit-trace compiler variant.

## Decisions resolved during promotion

Review froze the observable policies before roadmap creation:

1. **Output:** only `stacktrace:` rows follow the panic record; there is no
   redundant top-level `location:` row.
2. **Paths:** escaped provider-relative display paths provide stable,
   useful source identity.
3. **Enablement:** tracing is default-on and `--omit-runtime-trace` removes it
   completely at compilation.
4. **Context spelling:** semantic callable names identify source-owned
   lifecycle bodies and static initializers; initializer parameter signatures
   distinguish overloads without source-order ordinals.
5. **Depth/corruption defense:** execution has no separate trace-depth limit;
   rendering stops after 256 newest frames and marks an uncounted outer tail.
6. **Frame visibility and pop:** generated helpers remain attributed to their
   initiating source operation, and generated return paths use the unchecked
   two-instruction pop.

The [frozen design record](PANIC_RUNTIME_TRACE_DESIGN_PROPOSAL.md), active
[implementation roadmap](../roadmaps/PANIC_RUNTIME_TRACE_ROADMAP.md), and
living contracts are authoritative where they refine this earlier
investigation.

## Suggested implementation order

After the decisions above are frozen in living contracts, implementation can
proceed in reviewable layers:

1. define exact trace output, paths, context names, enablement, metadata
   structs, frame invariants, and the new runtime ABI version;
2. implement and directly test the hidden TLS state and allocation-free trace
   rendering without generated-code dependencies;
3. add explicit backend source/options input and deterministic semantic trace
   metadata construction;
4. integrate trace homes and inline TLS push/pop with frame planning and every
   return ABI, initially without interior location changes;
5. add call-site, generated-helper, allocation, explicit-panic, and static-
   termination location replacement with focused assembly tests;
6. add CLI omission policy and exact native golden traces, migrate affected
   stderr expectations, and document debugging behavior; and
7. measure enabled overhead, verify zero-cost omission, coalesce redundant
   stores if measurements justify it, then run the complete repository gates.

This order establishes the observable and ABI contracts before target code,
then proves frame lifetime before expanding location coverage. A future
implementation roadmap should split these outcomes into PR-sized tasks and
name the exact focused and repository-wide validation commands.
