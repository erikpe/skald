# Debugging the Compiler

Status: authoritative for compiler inspection workflows. Phase contracts and
MIR invariants belong to [Phases and IR](../compiler/PHASES_AND_IR.md); target
behavior belongs to the [Backend and Target Contract](../compiler/BACKEND.md).

## Inspect the nearest phase product

Start at the earliest incorrect product and move one boundary at a time.

| Product | Public renderer or command | Primary owner |
|---|---|---|
| Tokens | `lexer::dump_tokens` | lexer |
| Source AST | `syntax::dump_ast` | parser |
| Reachable module graph | `module::dump_module_graph` | entry selection and module loader |
| Resolved program | `resolve::dump_resolved` | resolver and stable identities |
| Typed HIR | `hir::dump_hir` | type checker and semantic operation selection |
| Preliminary MIR | `mir::dump_preliminary_mir` | unplanned static initializer bodies and publication boundaries |
| Static effects | `passes::static_lifecycle::dump_static_effects` | closed-world static access summaries and call/lifecycle witnesses |
| MIR | `mir::dump_mir` | target-independent lowering, storage, control flow, and cleanup |
| Diagnostics | `diagnostics::render_diagnostics` | diagnostic model, wording, spans, and source lookup |
| GNU assembly in Intel syntax | `backend::emit_assembly`, or `skac --emit asm` | selected backend |

The phase renderers are repository-internal Rust APIs. There are no CLI modes
for token, AST, resolved, HIR, preliminary MIR, static-effect, or MIR dumps.
Their text is a deterministic
debugging and regression format, not a stable interchange format.

When hand-built or future lowered MIR uses path-dependent state, the MIR dump
prints a `PathConditions` table before the block list. Each row identifies the
condition, optional parent, canonical activation storage, active and inactive
predecessors, and exact merge. A `path-condition` rvalue names both the
condition and activation storage. This makes an explicitly represented join
distinguishable from an ordinary join whose lifetime states disagree.

At a conditional full-expression boundary, follow each `path-condition`
value into its ordinary `branch`. The true successor contains the selected
cleanup or `storage-dead`, the false successor bypasses it, and both jump to a
small local merge before later cleanup continues. Nested cleanup tests parents
before children. After reverse cleanup and storage death have made every
alternative compatible, child activation storage ends inside its active
parent and root activation storage ends last. Assembly contains the same
ordinary loads, tests, branches, cleanup calls, and jumps; there is no runtime
conditional-cleanup operation.

Exact dump tests live with each phase. A focused search is usually enough:

```text
cargo test --locked -p skald-compiler token_dump
cargo test --locked -p skald-compiler ast_dump
cargo test --locked -p skald-compiler identities_and_dump_follow_canonical_module_order
cargo test --locked -p skald-compiler resolved_dump
cargo test --locked -p skald-compiler hir_dump
cargo test --locked -p skald-compiler mir_dump
cargo test --locked -p skald-compiler passes::static_lifecycle
```

These tests normally assert rather than print. While investigating, call the
public renderer from a nearby test, print with `eprintln!`, and add
`-- --nocapture`. Keep the final regression as an exact or focused structural
assertion and remove temporary output.

## Follow the pipeline

The intentional public phase paths are exercised by
[`public_api.rs`](../../crates/skald-compiler/tests/public_api.rs). For a source
failure, inspect lexing, parsing, resolution, and type checking in that order;
later products are not created after diagnostics from an earlier source phase.
For successful typed HIR, inspect MIR before assembly so semantic lowering and
target realization remain distinguishable.

For `std::error::panic`, the AST dump prints `IntrinsicFunction` and the
resolved dump prints `intrinsic Panic` beside its stable `FunctionId`.
Direct, selective, renamed-selective, aliased-module, and qualified uses must
all select that same ID. An unused declaration may appear as intrinsic
metadata in HIR and MIR without a definition or external link. An attempted
call statement should appear as `Panic` in HIR and `panic` in MIR, never as an
ordinary intrinsic call. `TYP041` indicates an invalid expression-position
use. Native assembly extracts the verified descriptor slice and calls
`ska_rt_panic` once.

For primitive comparisons, AST and resolved dumps retain the source predicate,
HIR records the exact integer or boolean operand kind, and MIR prints an
operation such as `lt.u64` or `eq.bool` with a `bool` result. Prefix logical
negation remains `LogicalNot` until type checking selects `LogicalNotBool`;
MIR prints `not.bool`. Signed versus unsigned target conditions first appear
in backend selection. For `(T) source` primitive casts, HIR and MIR record both
source and target types; pure MIR prints forms such as `cast.u64.i64`. A
checked `f64`-to-integer cast instead prints
`primitive-cast-range-check f64.T`, a success-only `checked-cast.f64.T`, one
result join, and a failure block ending in `primitive-cast-out-of-range`. When
a total cast feeds a signed array position, inspect the preceding source-level
unsigned comparison and its control-flow branch rather than looking for a
hidden checked conversion.

For `~`, `&`, `^`, and `|`, AST and resolved dumps retain source operator
identity and grouping. HIR prints exact forms such as
`BitwiseComplement.u8` and `BitwiseXor.u64`; MIR prints `not.u8` and
`xor.u64`. Each is an ordinary scalar rvalue with no operator-owned branch or
failure edge. A scalar spill around control-affecting right-operand lowering is
the existing eager binary evaluation rule, not a bitwise control effect.

For source `<<` and `>>`, AST and resolved dumps retain direction, grouping,
and spans. HIR prints `CheckedShift` with
`shl`, `sar`, or `shr`, the exact left type, `u64` count, width, and failure
capability. MIR must show secured left and count scalar spills, then a
`shift-count-check`; only its success block may contain the matching shift,
and its failure block must terminate with `shift-count-out-of-range`. In
assembly, the unsigned compare and valid-count branch must precede both the
first `rcx` count load and the `..., cl` instruction.

For source `/` and `%`, AST and resolved dumps retain operation identity,
grouping, and spans. HIR prints a checked integer-division operation with its
exact integer kind and operation-specific zero failure. MIR must show secured
dividend and divisor spills followed by an explicit zero check; only the
success block may perform `div` or `rem`, initialize the result carrier, and
join the enclosing expression. The failure block must terminate with
`integer-division-by-zero` or `integer-remainder-by-zero`. In signed assembly,
the zero and `i64::MIN / -1` guards must precede `idiv`; ordinary non-exact
results then pass through the floor-quotient correction.

For exact source `f64 / f64`, HIR and MIR print the portable `div.f64`
operation. There is no divisor check, failure block, or panic reason; assembly
uses scalar binary64 division. A floating zero divisor is therefore an
ordinary input whose infinity or NaN result can be inspected through canonical
standard-library formatting; backend-native fixtures remain available when a
test requires exact representation identity.

Source floating-comparison fixtures print `eq.f64`, `ne.f64`, `lt.f64`,
`le.f64`, `gt.f64`, or `ge.f64` in MIR. Their assembly must contain
`ucomisd`, a relation `setcc`, an explicit `setp` or `setnp` parity gate, byte
combination, and canonical zero extension. Mixed or otherwise unsupported
operand pairs diagnose before HIR; exact `f64` pairs appear throughout the
ordinary source phase dumps.

For `left && right` or `left || right`, AST and resolved dumps retain a
distinct logical node and HIR prints `Logical And` or `Logical Or`. MIR should
contain a `LogicalExpressions` row, a split branch, separate short and right
paths, one result carrier, and a join reload. Follow the row's path condition
into conditional cleanup: resources from a skipped right operand must never
become live, while selected resources remain live until reverse
full-expression cleanup. If a side effect or failure appears on a skipped
path, inspect logical lowering before the backend; assembly only realizes the
already verified CFG.

For `while`, the AST must retain the keyword, condition, body, and complete
statement span. The resolved dump assigns source-ordered callable-local
`LoopId`s, and HIR retains the structured loop plus its conservative
fallthrough effect. MIR then expands the statement into generic preheader,
condition, body, reachable latch, and exit blocks. Inspect condition
full-expression cleanup before the header branch, body-local `storage-live`
and `storage-dead` operations before the latch, and the ordinary backward
`goto` from latch to header. `break` and `continue` appear in resolved and HIR
dumps with their selected `LoopId`; MIR must clean each scope above that
loop's retained depth before an ordinary `goto` to the exit or latch,
respectively. A body with no latch-reaching path omits the unreachable latch.
Assembly should contain only the corresponding generic branches and jumps.

For `T(copy source)`, the AST and resolved dumps must retain a distinct copy
mode rather than an ordinary argument. HIR must show one selected copy
operation and checked exact-`T` source. MIR then shows any `checked_cast`
success/failure edge before one `copy_construct`, followed by the checked-view
and produced-temporary full-expression cleanup.

For `new T(copy source)`, that same source and checked-view sequence must
precede `shared-allocate`. MIR then names the unpublished allocation payload as
the destination of exactly one `copy-construct`, followed by
`shared-publish`, `shared-adopt`, checked-view end, and reverse
full-expression cleanup. Allocation before a required check or publication
before copy completion is malformed MIR.

For a produced read-only object alias argument, HIR records a `Produced` view
source rather than an existing place. MIR must create one exact-class
`Temporary` at the argument's source position, complete the producer into it,
and pass an ordinary `MirArgument::View` with exact complete-object origin.
The temporary remains live through later arguments and the call, then enters
reverse full-expression cleanup exactly once. Assembly should use the normal
three-component object-alias convention with no produced-source branch. A
corresponding `mut ref` argument must fail with `TYP020` before HIR. Compare
the `produced_alias_arguments` and `produced_alias_invalid_sources` goldens
when the failure crosses more than one phase.

For an explicitly dereferenced shared receiver or alias argument, HIR distinguishes a stable
`SharedPointee` from an `AnchoredSharedPointee` and retains the copied field or
adopted producer source. MIR declares each hidden owner as `shared-anchor`;
the corresponding `shared-copy`, `shared-field-copy`, call result, allocation
adopt, or shared cast must precede the consuming call, and `shared-release`
must follow it. Nested shared fields produce one anchor per owning edge.
Inline base and field projections remain beneath the same shared-pointee root.

For `(T) *source`, inspect both lifetimes. HIR shows the
`SelectedView` under a static or runtime `CheckedSource` or checked consumer.
MIR first creates any `shared-anchor`, then binds or checks a distinct
`checked-view` carrier. On a normal success path, the consumer must complete,
`end-checked-view` must appear, and only then may `shared-release` end the
anchor. A produced allocation may appear as an `exact` shared origin and fold
the target selection to a static binding even when an intervening shared view
has a broader static target.

For arrays, first distinguish the source-visible owner from its selected
backing. HIR records inline versus shared ownership, exact `ArrayTypeId`,
named-copy versus produced-adoption provenance, checked bounds, and anchor
kind. MIR then makes allocation, initialized prefixes, publication, loops,
projection checks, replacement, slice checks, and release explicit. A whole
inline alias follows the descriptor and observes replacement; an element or
nested-array alias follows a hidden backing anchor and keeps the old backing
live. Shared array access should show a stable, copied, adopted, or secured
owner anchor before the checked projection. MIR contains no header offsets or
strides; those first appear in x86-64 layout and instruction selection.

MIR verification runs at three boundaries:

1. immediately after HIR lowering in debug builds;
2. unconditionally at the input to `passes::run_mir_pipeline`; and
3. inside backend legality checking before target lowering.

A failure at the first boundary points to MIR production. A failure after a
pass points to the transformation or its input. Backend rejection beginning
with `input MIR failed verification` means malformed MIR reached the final
trust boundary; another structured backend error means verified MIR violates a
target-specific legality or lowering contract.

## Inspect runtime traces

The version-9 runtime renderer, x86-64 requested metadata planner, inline
activation-frame maintenance, source-call replacement, central reporter
failure replacement, and generated-helper/runtime attribution are implemented.
A directly constructed chain can isolate runtime formatting from backend
faults.

For metadata faults, run the focused `runtime_trace_metadata` tests and begin
with the existing MIR span at the reported operation; MIR deliberately
contains no trace instruction or
metadata identity. Then inspect backend planning for the source-callable
context, escaped provider-relative path, span-start line and column, frame
eligibility, and whether the operation requires a replacement.

Enabled x86-64 assembly gives an eligible source callable one linked 16-byte
record, one six-instruction publish sequence after incoming parameters are
preserved, and one two-instruction restore before the final result reload on
every normal return. A two-instruction replacement appears at each source
call, source operation that enters a panic-capable generated/runtime path, or
taken failure edge. Generated helpers and wrappers have no frame and inherit
the initiating source operation. `r11` is only a transient clobber; a
persistent trace register or a C maintenance call is a lowering defect.
Indirect calls require the trace replacement before the target is loaded into
the same scratch register.

Use ELF relocation and section inspection to distinguish a textual assembly
mistake from a link-model mistake. TLS access must use local-exec
`R_X86_64_TPOFF32` relocations to the hidden `ska_rt_trace_top`; context and
location records must be deterministic relocation-read-only data. A build
with `--omit-runtime-trace` must contain no trace symbol reference, metadata,
frame home, or maintenance instruction.

For incorrect output, reproduce first with the direct runtime harness using a
hand-built valid frame chain, then with the smallest native golden. The former
owns empty, nested, replaced, capped, and failed-write rendering; the latter
owns source-frame selection and operation attribution. A missing caller row
usually indicates an omitted push or premature pop. A correct caller with the
wrong line usually indicates replacement placement or span selection. A
generated helper name indicates a frame-eligibility defect. A crash while
walking a non-null link indicates compiler/runtime corruption rather than a
source panic.

Verifier tests use crate-visible `cfg(test)` fixture constructors and mutation
accessors such as `entries_mut_for_test`, `get_mut_for_test`, and
`remove_for_test`. They are intentionally unavailable to integration tests and
production callers. Use them to corrupt one invariant at a time rather than
constructing an accidentally invalid program with unrelated failures.

## Private initializer inspection

For `T(arguments)`, `new T(arguments)`, or `super(arguments)`, inspect the
resolved class declaration first. Each overload has its own visibility and
source-ordered `InitializerId`. Type checking determines applicability and a
unique most-specific identity before checking whether the current callable is
lexically owned by that initializer's exact declaring class. Consequently,
`TYP040` on a private overload does not mean the checker should retry a less
specific public overload.

An authorized HIR construction names the selected initializer but contains no
visibility metadata. An access failure produces no HIR. If changing only
resolved visibility changes successful HIR shape, or if private visibility
reappears in MIR or assembly, the phase boundary has been violated.

A derived initializer's `super(...)` is owned by the derived class, so it
cannot call a private base initializer. For `T[](length)` and
`shared T[](length)`, inspect the resolved type's stable zero-argument default
plan and then the array expression's type-check site: that consumer performs
the same exact-class authorization. Empty arrays and explicit-copy arrays do
not consult ordinary initializer visibility.

The focused checks are:

```text
cargo test --locked -p skald-compiler private_initializer
cargo test --locked -p skald-compiler --test pipeline_determinism private_initializer
make golden-filter GOLDEN_FILTER='**private_initializer**'
```

## Inspect assembly and native behavior

Emit assembly without linking:

```text
cargo run --locked -p skac -- input.ska --emit asm -o build/input.s
```

Compare the MIR operation, target legality/layout expectations, and emitted
instruction sequence. Internal labels and compiler-generated symbol spellings
are useful for comparison but are not an ABI promise. The
[driver guide](../compiler/DRIVER_AND_ARTIFACTS.md) owns toolchain selection,
runtime selection, artifact publication, diagnostics, and process status.

For complete behavior, use the golden runner. It keeps compiler observations
separate from link and execution observations and reports stdout, stderr, and
exit-status mismatches independently. Ordinary execution observes each stage
once; `--determinism compile` or `full` enables repeated compiler or complete
native checks. Use `--jobs 1` to remove process overlap while diagnosing a
case. Expectations may use exact or reviewed partial byte matches. Build
artifacts are under `build/golden/`; they are disposable debugging output.

Use `make golden-filter GOLDEN_FILTER='<glob>'` or
`make golden-exact GOLDEN_ID='<canonical-id>'` for common focused runs. Use
`scripts/golden.sh` when combining multiple filters, exclusions, report
formats, or debugging options. `scripts/golden.sh --explain '<canonical-id>'`
shows each resolved matcher name or index, policy, and inline or external
source. Add `--show-output` to a focused run to inspect the shared captured
stream and every passing matcher result. The
[golden fixture guide](../../tests/golden/README.md) is the authoritative
schema and matching reference.

## Symptom to owner

| Symptom | Inspect first |
|---|---|
| Wrong token boundary or literal spelling | token dump, then lexer |
| Wrong tree shape or recovery diagnostic | AST dump, then parser |
| Unknown, duplicate, or misbound name | resolved dump, then resolver |
| Wrong type, access, operation, or destination selection | HIR dump, then type checker |
| Wrong evaluation order, storage, branch, temporary, or cleanup | MIR dump, then MIR lowering/verifier |
| Correct MIR but rejected target operation or layout | backend legality and layout |
| Correct MIR but wrong instruction, call ABI, or address | backend lowering and assembly |
| Correct executable but wrong output or runtime failure | runtime ABI and direct runtime harness |
| Wrong CLI status, stream, path, tool invocation, or preserved output | driver and artifact tests |
| Output changes only across processes | identity allocation, unordered traversal, path rendering, or label generation |

For an exact diagnostic mismatch, compare the structured diagnostic before its
rendered text. For a native-only failure, reproduce at the narrowest available
layer: backend native unit test, golden executable, or direct C runtime harness.
The [testing guide](TESTING.md) explains where the resulting regression belongs.

For private standard-I/O intrinsics, inspect the MIR range-offset check and
backing anchor before reading assembly. On x86-64, open passes pointer and
length in `rdi`/`rsi`; read and write pass handle, pointer, and remaining length
in `rdi`/`rsi`/`rdx`. A larger offset must branch to array-bounds termination
before any `ska_rt_io_*` call. If those are correct, reproduce host-result
behavior with a public standard-I/O golden, focused native probe, or direct
runtime I/O harness.

## String pipeline inspection

For a string literal, start with the token and AST dumps to confirm decoded
bytes and the complete literal span. In a provider-aware request, the module
graph must contain one synthetic edge to `std::str` for each requiring module;
an explicit import coalesces with that edge but remains distinguishable in the
graph dump.

The resolved dump names one exact `StringLanguageItem` class and its three
field identities, followed by source-ordered literal-data identities. HIR
retains those identities on produced exact-class values and contains no
initializer, factory, or method-name lookup. MIR then shows immutable literal
data, `shared-static`, and `string-initialize` before ordinary copy,
assignment, argument/result, and cleanup operations.

In assembly, literal backing appears in immutable or relocation-read-only data
with the shared `u8[]` metadata relocation, `u64::MAX` strong-count sentinel,
exact decoded length, and bytes. Literal materialization must not call the
allocator or copy helper. Dynamic strings created by `std::str::Str` methods
instead use ordinary shared-array allocation and an exact-class call to the
private descriptor initializer. Slices call the same initializer with existing
backing and a checked subrange, followed by ordinary retain/release and
last-owner reclamation. For byte or slice-bound failures, inspect the
array-compatible one-time negative normalization relative to the descriptor
length, then the normalized signed ordering/bounds checks. Conversion to the
internal `u64` descriptor range occurs only after the normalized position is
known to be non-negative. The failing branch should end in `Panic` HIR and a
`panic` MIR terminator selected through the imported
`std::error::panic` identity; an array projection used only to provoke failure
indicates stale standard-library source.

Use the string-focused tests for the nearest reproduction:

```text
cargo test --locked -p skald-compiler strings
cargo test --locked -p skald-compiler --test pipeline_determinism string
make golden-filter GOLDEN_FILTER='**string**'
```

## Optional-value frontend inspection

Inspect optional frontend behavior at the narrowest owner defined by the
[compiler contract](../compiler/OPTIONAL_VALUES.md#dumps-and-diagnostics):

- token and AST dumps currently expose `?`, `!`, reserved `none`, contextual
  `some`, type-marker spans, presence tests, and unwrap nodes;
- resolved dumps expose canonical recursive optional identities, including
  optional and array payloads, plus absence, presence-test, and unwrap nodes;
- inline-optional HIR dumps expose absent/present initialization, field places,
  arguments/results, produced calls, copy, assignment, presence tests, and
  checked primitive extraction or bounded class payload views;
- inline-optional MIR dumps expose initialized optional places, caller-owned
  argument/result aggregates, explicit operations, unwrap success/failure
  blocks, checked-view begin/end operations, guarded-mutation branches, and
  exact non-returning failure reasons;
- assembly uses the documented state/payload offsets, recursive field layout,
  hidden destination ABI, inline guard counts, static failure reporting, and
  defensive traps for impossible verified state; and
- optional-container alias dumps use indirect optional places without object
  origin metadata; checked optional-array payload aliases additionally show a
  guarded payload projection and array anchor. Frozen shared optional boxes
  remain frontend diagnostics until their active roadmap moves the gate; and
- static-field HIR/MIR dumps expose optional shared owners through canonical
  identity-based static places, with no function-local storage carrier; final
  MIR also exposes planned activation, publication, and reverse-destruction
  regions.

For a static-field issue, first compare the declaration identity in the
resolved, HIR, and MIR dumps. In assembly, its target-private object should
appear once in `.bss` with `.zero`, declared alignment, and RIP-relative
address formation. An inherited class spelling, module alias, or second use
must not create another object symbol. The generated wrapper should call the
runtime-v8 marker, the private program initializer, and Skald entry in that
order, then preserve the entry result across the private program finalizer.
Neither lifecycle function nor a field initializer may be exported with
`.globl`, and ordinary field access should contain no lifecycle-state guard.
