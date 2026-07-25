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
| Resolved program | `resolve::dump_resolved` | resolver and stable identities |
| Typed HIR | `hir::dump_hir` | type checker and semantic operation selection |
| MIR | `mir::dump_mir` | target-independent lowering, storage, control flow, and cleanup |
| Diagnostics | `diagnostics::render_diagnostics` | diagnostic model, wording, spans, and source lookup |
| GNU assembly in Intel syntax | `backend::emit_assembly`, or `skac --emit asm` | selected backend |

The phase renderers are repository-internal Rust APIs. There are no CLI modes
for token, AST, resolved, HIR, or MIR dumps. Their text is a deterministic
debugging and regression format, not a stable interchange format.

Exact dump tests live with each phase. A focused search is usually enough:

```text
cargo test --locked -p skald-compiler token_dump
cargo test --locked -p skald-compiler ast_dump
cargo test --locked -p skald-compiler resolved_dump
cargo test --locked -p skald-compiler hir_dump
cargo test --locked -p skald-compiler mir_dump
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

MIR verification runs at three boundaries:

1. immediately after HIR lowering in debug builds;
2. unconditionally at the input to `passes::run_mir_pipeline`; and
3. inside backend legality checking before target lowering.

A failure at the first boundary points to MIR production. A failure after a
pass points to the transformation or its input. Backend rejection beginning
with `input MIR failed verification` means malformed MIR reached the final
trust boundary; another structured backend error means verified MIR violates a
target-specific legality or lowering contract.

Verifier tests use crate-visible `cfg(test)` fixture constructors and mutation
accessors such as `entries_mut_for_test`, `get_mut_for_test`, and
`remove_for_test`. They are intentionally unavailable to integration tests and
production callers. Use them to corrupt one invariant at a time rather than
constructing an accidentally invalid program with unrelated failures.

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

For complete behavior, use the golden runner. It keeps deterministic assembly
or diagnostic checks separate from link and execution observations, compares
two native executions, and reports stdout, stderr, and exit-status mismatches
independently. Build artifacts are under `build/golden/`; they are disposable
debugging output.

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
