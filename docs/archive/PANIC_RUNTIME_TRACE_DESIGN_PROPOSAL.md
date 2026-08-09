# Panic Runtime Trace Design Record

Status: frozen design; ready for an implementation roadmap, but not yet
implemented. The completed
[runtime trace investigation](PANIC_RUNTIME_TRACE_INVESTIGATION.md) provides
the Skald and Niflheim implementation evidence behind this design.

This design adds source locations and a shadow call stack to Skald panic
output while holding successful-execution cost to a small fixed sequence of
inline Linux x86-64 instructions. It deliberately avoids heap-backed trace
storage, per-location runtime calls, and a permanently reserved register.

The language and implementation-dependent choices in this record are frozen.
Current implemented behavior remains distinguished from the frozen extension
in
[Errors and Exceptional Control Flow](../language/ERRORS.md#frozen-panic-runtime-traces),
[Compiler Phases and IR](../compiler/PHASES_AND_IR.md#frozen-runtime-trace-phase-boundary),
[Backend and Target Contract](../compiler/BACKEND.md#frozen-runtime-trace-target-boundary),
and
[Runtime ABI](../compiler/RUNTIME_ABI.md#frozen-runtime-trace-abi-version-9),
which now own the living contracts.

## Intended outcome

When runtime tracing is enabled, every source callable activation contributes
one live trace frame containing its current source location. Entering a source
callable pushes that frame, reaching a panic-observable source location
replaces the frame's location pointer, and every normal return pops the frame.
Panic leaves the active frames in place and renders them newest first.

On Linux x86-64 with the local-exec TLS model, the frozen steady-state
instrumentation is:

- six instructions to push one frame;
- two instructions to pop it;
- two instructions to replace its location; and
- no C call, heap allocation, capacity check, or permanently reserved general
  register on any of those paths.

Trace-disabled compilation emits no trace frame homes, instructions, metadata,
or TLS references.

## Current boundary

Skald currently writes exactly `panic: `, the length-delimited message bytes,
and one line feed, then exits without unwinding or remaining cleanup. The
runtime reporter is allocation-free and writes directly to the standard-error
descriptor. Explicit panic, compiler-known MIR termination, valid host
allocation failure, and legal ownership-count overflow share this reporter.
Compiler/runtime defects remain silent hard traps.

The compiler already retains the information needed by this design:

- `SourceDatabase` maps a `Span` start to a one-based line and Unicode-scalar
  column;
- MIR definitions, instructions, calls, blocks, and terminators retain spans;
- explicit `Panic` and every `Terminate` reason retain the exact failure span;
- declarations retain module ownership and source names; and
- module provenance retains logical and root-relative source paths.

Backend emission currently accepts only final verified `MirProgram`, even
though the driver still owns the corresponding `SourceDatabase`. Trace
metadata therefore requires an explicit backend input extension, not new
source syntax or source text copied into MIR.

## Scope and invariants

The frozen design includes:

- source-callable shadow frames on Linux x86-64;
- direct local-exec TLS access to the current frame pointer;
- deterministic static context and location metadata;
- source call-site and failure-site location selection;
- allocation-free panic stack rendering;
- a compile-time zero-overhead omission mode; and
- exact runtime, backend, driver, and native test ownership.

The following invariants apply:

1. Panic remains non-returning, non-unwinding, and uncatchable.
2. Trace maintenance never allocates and cannot introduce a new source-level
   failure.
3. No general-purpose register is permanently reserved for tracing.
4. A trace frame lives in the same native frame as its source activation.
5. Generated implementation helpers do not create user-visible stack frames.
6. A caller records its active call site before a source callee pushes.
7. Hard traps do not acquire panic output or trace rendering.
8. Omitted tracing has zero generated execution and metadata cost.
9. Paths, context names, record order, assembly, and stderr are deterministic.
10. The runtime learns only the trace ABI records, not Skald strings, MIR,
    modules, `SourceId`, or object layouts.

This design does not include:

- recoverable exceptions, unwinding, handlers, or exceptional cleanup;
- signal or hard-trap stack reporting;
- native frame-pointer, DWARF, or external-symbolizer stack walking;
- tracing of arbitrary foreign C frames;
- sampling, profiling, coverage, logging, or general debug events;
- a Linux AArch64 implementation; or
- a source-level API for inspecting or mutating trace state.

The representation is intended to remain realizable on a future Linux
AArch64 backend through target-specific TLS addressing, but that work is not
part of this design's first implementation scope.

## Decision register

Every row is frozen for implementation. Reopening one requires an explicit
contract revision before dependent implementation proceeds.

| ID | Decision | Frozen direction | State |
|---|---|---|---|
| [TR1](#tr1--trace-frame-and-tls-state) | Dynamic trace state | One linked frame in each source native frame; one hidden thread-local top pointer | **Frozen** |
| [TR2](#tr2--inline-x86-64-instrumentation) | Hot-path realization | Inline local-exec TLS push/pop and direct location replacement with `r11` as transient scratch | **Frozen** |
| [TR3](#tr3--source-context-boundary) | Visible frames | Source functions and source-owned lifecycle/static-initializer bodies only; omit generated helpers and wrappers | **Frozen** |
| [TR4](#tr4--location-update-boundary) | Update sites | Calls, panic-capable runtime operations, and failure-only reporter edges rather than every MIR instruction | **Frozen** |
| [TR5](#tr5--static-metadata) | Trace records | Interned length-delimited context/path bytes and fixed-width context/location records | **Frozen** |
| [TR6](#tr6--context-names-and-source-paths) | Human-facing identity | Semantic callable names plus escaped provider-relative paths | **Frozen** |
| [TR7](#tr7--panic-output) | Rendering | Existing panic record followed by non-duplicated newest-first `stacktrace:` rows | **Frozen** |
| [TR8](#tr8--enablement) | Compilation policy | Enabled by default; `--omit-runtime-trace` provides zero-cost omission | **Frozen** |
| [TR9](#tr9--depth-and-defect-boundary) | Depth and corruption | No execution depth limit; render at most 256 frames and mark omitted outer frames | **Frozen** |
| [TR10](#tr10--compiler-and-abi-ownership) | Phase and ABI boundary | Backend receives sources/options explicitly; runtime ABI advances to version 9 while `ska_rt_panic` keeps its signature | **Frozen** |
| [TR11](#tr11--performance-and-verification) | Acceptance evidence | Exact hook counts, complete native traces, deterministic output, and measured enabled/omitted overhead | **Frozen** |
| [TR12](#tr12--promotion-boundary) | Design maturity | Freeze all rows and promote the contracts before creating an implementation roadmap | **Complete** |

## Frozen design

### TR1 — Trace frame and TLS state

Each traced source callable receives one 16-byte frame in its ordinary fixed
native stack allocation:

```c
typedef struct SkaRtTraceContext SkaRtTraceContext;
typedef struct SkaRtTraceLocation SkaRtTraceLocation;
typedef struct SkaRtTraceFrame SkaRtTraceFrame;

struct SkaRtTraceFrame {
    SkaRtTraceFrame* previous;
    const SkaRtTraceLocation* location;
};
```

The runtime defines one zero-initialized thread-local top pointer:

```c
extern _Thread_local SkaRtTraceFrame* ska_rt_trace_top;
```

The compiler and runtime treat the symbol as hidden and non-preemptible. The
generated executable links the C11 runtime statically, allowing the Linux
x86-64 local-exec TLS model. The reporter reads the same thread-local pointer
when panic begins.

The frame is linked rather than stored in a runtime vector. Its storage and
lifetime therefore already match the source activation, recursion consumes no
separate heap, and tracing cannot fail because a diagnostic buffer needs to
grow. Panic does not unwind, so every active linked frame remains valid while
the reporter walks it.

The direct pop is trusted generated code: it restores `previous` without a
runtime top-equality check. An invalid frame chain is a compiler/runtime
defect, not a source panic. MIR verification, frame-layout tests, assembly
tests, and native nesting tests own this invariant.

### TR2 — Inline x86-64 instrumentation

The x86-64 backend uses local-exec `R_X86_64_TPOFF32` relocations for direct
TLS access. With `previous` at frame offset zero and `location` at offset
eight, representative Intel-syntax sequences are:

A focused GNU assembler/linker probe during this design confirmed that the
shown `fs:symbol@tpoff` form emits `R_X86_64_TPOFF32` and links a hidden C11
TLS definition into the toolchain's default position-independent executable.

```text
# Push: six instructions.
mov r11, qword ptr fs:ska_rt_trace_top@tpoff
mov qword ptr [rbp + trace_previous_home], r11
lea r11, [rip + location_record]
mov qword ptr [rbp + trace_location_home], r11
lea r11, [rbp + trace_previous_home]
mov qword ptr fs:ska_rt_trace_top@tpoff, r11

# Replace: two instructions.
lea r11, [rip + location_record]
mov qword ptr [rbp + trace_location_home], r11

# Pop: two instructions.
mov r11, qword ptr [rbp + trace_previous_home]
mov qword ptr fs:ska_rt_trace_top@tpoff, r11
```

The counts exclude the ordinary frame prologue/epilogue that Skald already
emits. They are a target-design objective rather than a portable language
guarantee.

`r11` is transient scratch, not a reserved trace register. Current lowering
already treats values as having stack homes. A future register allocator must
model each trace sequence's scratch clobber and schedule it where `r11` or
another caller-saved register is dead. The intended sites naturally support
that:

- push follows incoming-parameter preservation and precedes body values;
- pop precedes final result reload from its frame home;
- call-site replacement occurs at a call boundary with caller-saved clobbers;
  and
- failure replacement precedes a non-returning reporter call.

All general-purpose registers, including `r15`, remain available to future
allocation outside these short sequences. A trace-disabled build contains no
additional scratch constraint.

### TR3 — Source context boundary

One trace frame represents one source-authored executable context:

- top-level functions;
- instance and static methods;
- ordinary initializers;
- user copy constructors and copy assignments;
- user destructors; and
- explicit static-field initializer bodies.

The process entry wrapper, static lifecycle coordinator, array helpers,
ownership helpers, synthesized copy/finalization helpers, and target-private
thunks do not push frames. Before entering such a helper, its source caller
records the operation location. If the helper invokes a user body, that user
body pushes its own source context normally.

External C functions cannot push Skald frames. Their caller records the
external call site before crossing the ABI. If foreign code terminates or
does not return, existing trust-boundary behavior remains unchanged.

### TR4 — Location update boundary

The active location changes only where panic can observe a materially new
source operation:

- before every direct, virtual, interface, static, lifecycle, or external
  call;
- before every target-generated helper call made for a source operation;
- before runtime operations permitted to call `ska_rt_panic` internally,
  currently `ska_rt_alloc`;
- in the failure block immediately before explicit dynamic-message panic;
- in the failure block immediately before every static MIR termination
  reporter; and
- before any future runtime operation whose contract permits reporting.

Failure-only placement keeps successful checked casts, optional access,
bounds checks, division, remainder, shifts, and primitive casts free of a
location store. Pure operations and hard-trap-only checks need no update
because the reporter cannot observe them.

The caller update must execute before the callee's push. This yields a trace
whose top frame is the callee failure site and whose next frame is the exact
caller call site. Argument production or caller-side copy construction that
fails before callee entry remains attributed to its own source operation.

Target lowering carries the current MIR instruction or terminator span while
selecting nested helper calls. A later target-private dataflow optimization
may remove a replacement when all incoming paths already establish the same
record. Correctness must not depend on that optimization.

### TR5 — Static metadata

The runtime-visible metadata is fixed-width and length-delimited:

```c
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
```

On Linux x86-64 these occupy 32 and 24 bytes respectively. Using `uint64_t`
avoids imposing a new source-size validity limit merely because tracing is
enabled. The stack stores only an eight-byte location pointer per active
frame, in addition to its eight-byte previous pointer.

The backend emits deterministic interned pools for:

1. used callable-name and source-path byte strings;
2. one context per traced source callable; and
3. one location per distinct traced callable and span-start line/column.

Pointer-bearing records belong in relocation-read-only target metadata. Pool
order follows deterministic semantic identities and locations, never address
values or hash iteration. Unused records are not emitted.

### TR6 — Context names and source paths

Context names derive from semantic module and declaration metadata rather than
native mangling. The spellings are:

```text
app::main
app::Widget.make
app::Widget.init(i64, ref app::Config)
app::Widget.copy
app::Widget.assign
app::Widget.destroy
app::Widget.cache::<static-init>
```

Initializer parameter types distinguish overloads without source-order
ordinals that change when another overload is inserted. Parameter modes are
included where needed for unambiguous source identity. Generated helper names
never enter this vocabulary.

Loaded modules use their provenance `root_relative_path`, producing stable
paths such as `app/main.ska` and `std/str.ska` without temporary provider-root
prefixes. A positional outside-root entry uses its configured relative display
spelling when available; an absolute outside-working-directory spelling
remains absolute unless review selects a separate redaction policy.

Paths are escaped at compile time into one display line. Backslash, newline,
carriage return, tab, control bytes, and non-UTF-8 host bytes must not forge
trace structure. Context identifiers are already ASCII by the implemented
grammar. Both fields remain length-delimited in the ABI even after rendering.

### TR7 — Panic output

The existing panic record remains first. When at least one trace frame is
active, the reporter appends:

```text
panic: configuration is missing
stacktrace:
  at app::load (app/load.ska:18:9)
  at app::main (app/main.ska:7:5)
```

The newest frame appears first. There is no separate `location:` row because
it would duplicate the first stack row. A reporter called without an active
Skald frame preserves the current single-line output.

Rendering reuses the existing retrying direct writes. It performs no heap
allocation or C buffered I/O, converts line and column with fixed local
buffers, and terminates immediately if any write fails. The panic payload
remains raw length-delimited data, so its embedded zeroes and newlines retain
their current behavior.

### TR8 — Enablement

Trace emission is enabled by default for native executable and
assembly output. `--omit-runtime-trace` disables the complete feature at
compile time.

Omission removes:

- the 16-byte trace frame home;
- TLS push/pop instructions;
- location replacement instructions;
- trace contexts, locations, and their strings; and
- any backend source-location work required only by tracing.

The runtime library may still contain its trace ABI and reporter support; a
program that publishes no frame observes the current one-line panic record.
There is no environment-variable or runtime flag whose value could make
otherwise identical executions nondeterministic while retaining instrumentation
overhead.

### TR9 — Depth and defect boundary

The linked representation imposes no separate trace-depth limit during
execution. Native stack exhaustion remains the practical recursion boundary.

Panic renders at most 256 newest frames. If another valid `previous` pointer
remains, it emits:

```text
  ... outer frames omitted
```

It need not walk or count the omitted tail. This bounds panic output and also
prevents a corrupted cycle from looping indefinitely after 256 rows. An
invalid non-null pointer can still cause a hard process failure while being
read; such corruption is a compiler/runtime defect and does not require a
second diagnostic mechanism.

The compiler emits no dynamic top-equality or underflow check on pop because
that would exceed the two-instruction path and guard a compiler-owned
invariant. Focused generated-assembly and native nesting tests must make an
incorrect pop difficult to introduce silently.

### TR10 — Compiler and ABI ownership

Backend emission receives an explicit input containing:

- the final verified `MirProgram`;
- read-only `SourceDatabase` access when tracing is enabled; and
- backend trace policy.

MIR continues to own semantic spans. It does not gain target record IDs,
rendered paths, or TLS operations. The backend resolves used span starts once,
constructs target records, plans trace frame homes, selects inline sequences,
and emits the metadata.

The runtime owns the C record layouts, the hidden thread-local top symbol, and
allocation-free rendering. These additions require a runtime ABI version and
link-marker transition. The reporter keeps its existing signature:

```c
_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length);
```

Keeping that signature allows `ska_rt_alloc` and compiler-generated static
failures to continue using the same reporting call. The reporter discovers
the active trace through TLS.

Linux x86-64 instruction selection owns `fs:` addressing and ELF TLS
relocations. A future Linux AArch64 backend may realize the same target-
independent frame semantics through `TPIDR_EL0` and its ELF TLS relocations,
but this design does not freeze AArch64 instruction counts or implementation
work.

### TR11 — Performance and verification

The enabled design adds, per traced source activation:

- 16 fixed stack-frame bytes;
- six push instructions;
- two pop instructions; and
- two instructions at each executed relevant location change.

A typical source call therefore executes ten trace instructions: two at the
caller call site and eight in the callee. Pure loops without calls,
panic-capable runtime operations, or taken failure edges pay no repeated trace
work beyond their containing activation.

Acceptance must measure rather than infer the cost. Benchmarks should include
tiny call-heavy recursion, a pure tight loop, allocation-heavy execution, and
representative golden programs, each with tracing enabled and omitted. The
first implementation need not promise a numeric overhead ceiling, but a
material regression must be understood before default-on tracing ships.

Verification ownership is:

- source/source-database tests for line and Unicode-column mapping;
- backend metadata tests for interning, path escaping, semantic context names,
  deterministic ordering, sections, and relocations;
- frame and assembly tests for exact push/pop/update placement, scratch
  clobbers, every return ABI, helper suppression, and zero-cost omission;
- runtime C tests for empty, single, nested, replaced, capped, and failed-write
  traces with exact bytes and no allocation;
- native goldens for direct, recursive, virtual/interface, lifecycle, static-
  initialization, explicit panic, every static termination family, allocation
  failure, and ownership overflow; and
- cross-process determinism tests that compile under different temporary roots.

The ordinary repository gates remain `make check`, `make msrv-check`, and
`git diff --check`, with focused runtime, backend, driver, and golden commands
defined by the eventual implementation roadmap.

### TR12 — Promotion boundary

The pre-roadmap promotion boundary is complete:

1. every decision row is confirmed;
2. the complete design is promoted into the language error, compiler phase,
   backend, runtime ABI, driver, debugging, testing, and status documents;
3. the implemented grammar was checked and remains unchanged;
4. exact stderr examples and omission behavior agree across contracts;
5. runtime ABI version 9 and marker `ska_rt_abi_v9` are selected for the
   incompatible trace-state addition; and
6. this proposal and its supporting investigation are archived as durable
   design records.

An implementation roadmap may now divide work by contract, runtime
foundation, metadata/frame representation, location coverage, CLI/goldens,
performance, and closeout without reopening these choices.

## Resolved review decisions

Review confirmed direct hidden local-exec TLS access, default-on emission with
compile-time omission, escaped provider-relative paths, semantic initializer
signatures, a 256-row rendering cap with an uncounted omitted-tail marker,
omission of all generated lifecycle helpers, and the two-instruction unchecked
pop. Ordinary source-authored standard-library and lifecycle bodies remain
visible; only generated helpers, wrappers, runtime C frames, and the bodyless
panic intrinsic are omitted.
