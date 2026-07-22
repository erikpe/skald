# Documentation Overhaul Migration Inventory

Status: active; audited through driver, artifacts, and contributor workflows.

This inventory gives every living heading and repository reference a planned
destination. It is a migration ledger, not an additional authority for the
facts being moved. Planned destination paths are written as code until those
documents exist, which keeps repository links valid at every intermediate
step.

## Document disposition

| Current document | Disposition |
|---|---|
| `README.md` | DOC16 reduces it to project identity, quickstart, a short capability summary, and links to focused authorities. |
| `docs/README.md` | DOC0 makes it the durable authority, maturity, navigation, linking, history, and verification policy. Later tasks replace its temporary legacy-authority list with the focused structure. |
| `docs/language/README.md` | Retain as the broad language specification and navigation entry point; later language tasks add focused semantic links without turning it into another feature inventory. |
| `docs/language/STATUS.md` | Retain as the sole feature-maturity and compiler-support matrix; update it whenever source behavior, target availability, or a future design state changes. |
| `docs/language/GRAMMAR.md` | Retain as the sole exact accepted-syntax authority; update it with lexer/parser and source-shape tests whenever syntax changes. |
| `docs/language/TYPES_AND_VALUES.md` | Retain as the authority for implemented types, literals, value/place distinctions, expressions, operators, conversion availability, and future value-family maturity links. |
| `docs/language/FUNCTIONS_AND_CONTROL_FLOW.md` | Retain as the authority for callable declarations, parameters, calls, lexical scopes, statements, returns, definite-return analysis, and source-visible evaluation order. |
| `docs/language/CLASSES_AND_LIFECYCLE.md` | Retain as the authority for exact classes, member namespaces, inline containment, receivers, initialization, object places, copying, assignment, materialization, and deterministic lifecycle behavior. |
| `docs/language/ALIASES_AND_OWNERSHIP.md` | Retain as the authority for implemented exact-class alias parameters, access, forwarding, overlap, non-escape, and the explicit maturity boundary around future ownership forms. |
| `docs/language/POLYMORPHISM.md` | Retain as the exploratory language-design authority for inheritance, lifecycle composition, polymorphic views, dispatch, interfaces, type tests, and checked narrowing; PM0 freezes its open profile choices there. |
| `docs/language/MODULES_AND_INTEROP.md` | Retain as the authority for the implemented compilation unit, top-level namespace, entry point, primitive external declarations, trust boundary, unsupported external forms, and open future module design. |
| `docs/language/ERRORS.md` | Retain as the authority for language-level compile-time rejection, current runtime-failure boundaries, normal-flow cleanup limits, and future exceptional-control-flow constraints. |
| `docs/compiler/README.md` | Retain as the authority for durable compiler architecture, repository roles, compiler crate API policy, and extension policy. |
| `docs/compiler/PHASES_AND_IR.md` | Retain as the authority for phase products, target-independent IR responsibilities, verification, deterministic dumps, and trust boundaries. |
| `docs/compiler/BACKEND.md` | Retain as the authority for backend dispatch, target legality, x86-64 System V layout and ABI realization, frames, instruction selection, symbols, and assembly emission. |
| `docs/compiler/RUNTIME_ABI.md` | Retain as the authority for the public C runtime surface, version and link guard, platform requirements, bootstrap output records, failure behavior, and current responsibility boundary. |
| `docs/compiler/DRIVER_AND_ARTIFACTS.md` | Retain as the authority for compiler orchestration, CLI modes, target/toolchain/runtime selection, input protection, artifact publication, and driver failure boundaries. |
| `docs/development/README.md` | Retain as the authority for contributor prerequisites, Makefile entry points, supported Rust toolchains, MSRV use, and external clean-checkout automation. |
| `docs/SKALD_DRAFT_SPEC.md` | DOC1-DOC10 verify and migrate its retained language claims; DOC13 moves runtime claims; DOC16 removes the superseded monolith. |
| `grammar/README.md` | Retain only as a pointer to `docs/language/GRAMMAR.md` while historical links still target the old path; DOC16 removes it. |
| `docs/REPO_STRUCTURE.md` | DOC11-DOC14 moved compiler, backend, runtime, driver, and workflow contracts; DOC15 moves its remaining testing contract, and DOC16 removes the superseded migration document. |
| `docs/NEXT_SLICE_BOUNDARIES.md` | DOC1 and DOC8-DOC11 move stable design maturity and extension policy; roadmap ordering remains in `docs/roadmaps/README.md`; DOC16 removes the duplicate. |
| `docs/DEBUGGING.md` | DOC15 verifies and moves its workflow to `docs/development/DEBUGGING.md`; DOC16 removes the old path. |
| `samples/README.md` | Retain as the concise sample catalog; link language behavior to focused authorities when those exist. |
| `scripts/README.md` | Retain as concise script ownership guidance linking to the Makefile-owned development workflow. |
| `std/README.md` | Retain as the current standard-library status note; later link runtime and language authorities without speculating about an unimplemented library. |
| `tests/README.md` | DOC15 slims it to top-level test placement and links to `docs/development/TESTING.md`. |
| `tests/compiler/README.md` and `tests/compiler/robustness/README.md` | Retain as concise corpus mechanics; DOC15 moves general testing policy to the development guide. |
| `tests/golden/README.md` | Retain golden discovery, sidecar, and harness mechanics; DOC15 removes duplicate feature claims. |
| `tests/runtime/README.md` | DOC13 keeps only runtime-harness mechanics and links the runtime ABI authority; DOC15 aligns general test guidance. |
| `docs/roadmaps/README.md` | Retain as the only active-plan and dependency index. |
| Active roadmap and discovery documents under `docs/roadmaps/` | Retain while actionable, then archive completed roadmaps and keep only pending discoveries indexed. |
| Documents under `docs/archive/` | Preserve as history; DOC16 repairs links to new living authorities without rewriting historical prose. |
| `.codex/skills/*/SKILL.md` | Retain as repository workflow instructions; update authority paths when the focused structure replaces legacy paths. |

## Root and navigation headings

| Source heading | Intended authority or action |
|---|---|
| `README.md` — “Skald” | Retain in the reduced root README. |
| “Language direction” | `docs/language/README.md`; maturity details in `docs/language/STATUS.md`. |
| “Implemented language” | `docs/language/STATUS.md`, with semantic links. |
| “Compiler design” | `docs/compiler/README.md`. |
| “Building and using skac” | Quickstart stays in root; exact driver and artifact behavior moves to `docs/compiler/DRIVER_AND_ARTIFACTS.md`; contributor prerequisites move to `docs/development/README.md`. |
| “Future work” | `docs/language/STATUS.md` and the active roadmap index. |
| “History” | Retain in the root README and link archive history. |
| “Documentation” | Retain as a short link to `docs/README.md`. |
| `docs/README.md` — “Skald Documentation” | Retain as the documentation entry point. |

## Draft language specification headings

| Source heading or headings | Intended authoritative destination |
|---|---|
| “Skald Draft Language Specification”; “1. Purpose and Scope”; “2. Design Summary” | `docs/language/README.md`; support and maturity claims move to `docs/language/STATUS.md`. |
| “3. Source Files, Modules, and Visibility”; “3.1 Restricted Bootstrap External Functions” | `docs/language/MODULES_AND_INTEROP.md`; toolchain/linkage mechanics move to compiler documents. |
| “4. Types and Binding Modes”; “4.1 Primitive Types”; “4.1.1 Numeric Literals” | `docs/language/TYPES_AND_VALUES.md`. |
| “4.2 Object Types” | `docs/language/CLASSES_AND_LIFECYCLE.md`; polymorphic type direction moves to `docs/language/POLYMORPHISM.md`. |
| “4.3 Shared Types” | `docs/language/ALIASES_AND_OWNERSHIP.md`; status in `docs/language/STATUS.md`. |
| “4.4 Universal Root Type” | `docs/language/POLYMORPHISM.md`; status in `docs/language/STATUS.md`. |
| “4.5 Alias Binding Modes”; “4.5.1 Borrow Anchors”; “4.5.2 Deferred Local Alias Bindings” | `docs/language/ALIASES_AND_OWNERSHIP.md`. |
| “4.6 Optional Types”; “4.7 Array Types”; “4.8 Str” | Non-implementation and unresolved maturity link through `docs/language/TYPES_AND_VALUES.md` to `docs/language/STATUS.md`; DOC10 prunes the premature sketches. |
| “5. Declarations”; “5.1 Local Variables”; “5.2 Functions”; “5.3 Function Values” | `docs/language/FUNCTIONS_AND_CONTROL_FLOW.md`; function-value maturity in `docs/language/STATUS.md`. |
| “5.4 Classes”; “5.4.1 Instance-Method Receiver Mutability”; “5.4.2 Restricted Stage-0 Inline-Object Profile” | `docs/language/CLASSES_AND_LIFECYCLE.md`; historical profile names are removed. |
| “5.4.3 Restricted Stage-0 Alias-Parameter Profile” | Source semantics move to `docs/language/ALIASES_AND_OWNERSHIP.md`; call ABI moves to `docs/compiler/BACKEND.md`. |
| “5.4.4 Frozen Class-Typed Inline-Field Profile”; “Field declarations and containment”; “Direct field construction and liveness”; “Nested places, receivers, and aliases”; “Evaluation and phase boundaries”; “Boundary with later object-model slices” | Language rules move to `docs/language/CLASSES_AND_LIFECYCLE.md`; alias rules link to `ALIASES_AND_OWNERSHIP.md`; compiler boundaries move to `docs/compiler/PHASES_AND_IR.md`. |
| “5.4.5 Frozen Local Deterministic-Destruction Profile”; “Declaration and body contract”; “Lifetime registration and normal cleanup”; “Complete-object and field order”; “Diagnostics and phase boundary”; “Exclusions and extension boundary” | Source lifetime rules move to `docs/language/CLASSES_AND_LIFECYCLE.md`; phase details move to `docs/compiler/PHASES_AND_IR.md`; maturity moves to `docs/language/STATUS.md`. |
| “5.4.6 Frozen Exact-Class Object-Value Profile”; “Lifecycle declarations and identities”; “Copy capabilities and synthesis”; “Local initialization, assignment, and aliases”; “Value parameters and arguments”; “Object results and return storage”; “Temporaries and full expressions”; “Permitted copy elision and observable effects”; “Diagnostics and extension boundary” | Source-visible behavior moves to `docs/language/CLASSES_AND_LIFECYCLE.md`, with focused links to function and alias semantics; compiler identities and lowering move to compiler documents. |
| “5.5 Initialization Members”; “5.6 Copy Constructors and Copy Assignment”; “5.7 Destruction Members”; “6. Assignment, Copying, and Object Lifetime”; “6.1 Optional Copy Elision”; “6.2 Assignment to Parameters” | `docs/language/CLASSES_AND_LIFECYCLE.md`, with parameter rules linked to `FUNCTIONS_AND_CONTROL_FLOW.md`. |
| “7. Heap Allocation and Shared Ownership” | Source ownership guarantees move to `docs/language/ALIASES_AND_OWNERSHIP.md`; allocation/runtime mechanics move to `docs/compiler/RUNTIME_ABI.md`; maturity moves to `STATUS.md`. |
| “8. Classes, Inheritance, and Polymorphism”; “8.1 Inline Values and Slicing”; “8.2 Shared Upcasts”; “8.3 Alias-Parameter Upcasts”; “8.4 Virtual Dispatch”; “9. Interfaces” | `docs/language/POLYMORPHISM.md`, with ownership links to `ALIASES_AND_OWNERSHIP.md`. |
| “10. Expressions and Statements”; “10.3 Operators” | Expression/value rules move to `docs/language/TYPES_AND_VALUES.md`; statement and order rules move to `FUNCTIONS_AND_CONTROL_FLOW.md`. |
| “10.1 Conditional Statements”; “10.2 Returns and Call Statements” | `docs/language/FUNCTIONS_AND_CONTROL_FLOW.md`. |
| “10.4 Indexing, Slicing, and For-In” | Unsupported value/container status links through `docs/language/TYPES_AND_VALUES.md`; control flow moves to `FUNCTIONS_AND_CONTROL_FLOW.md`; DOC10 prunes unresolved syntax and protocol sketches. |
| “11. Casts, Type Tests, and Equality” | Conversions and equality move to `docs/language/TYPES_AND_VALUES.md`; hierarchy tests/narrowing move to `POLYMORPHISM.md`. |
| “12. Error Model”; “12.1 Checked Exceptions” | `docs/language/ERRORS.md`, with unresolved maturity in `STATUS.md`. |
| “13. Runtime Model”; “13.1 Bootstrap i64 Output”; “13.2 Bootstrap bool Output”; “13.3 Bootstrap Remaining-Primitive Output” | C contract moves to `docs/compiler/RUNTIME_ABI.md`; source declarations move to `MODULES_AND_INTEROP.md`. |
| “13.4 Stage-0 Inline-Object Layout and Receiver ABI”; “13.5 Stage-0 Alias-Parameter ABI” | `docs/compiler/BACKEND.md`; source semantics link back to class and alias authorities. |
| “14. Relationship to Niflheim” | A concise non-authoritative history/design note in `docs/language/README.md`; Skald documents remain authoritative. |
| “15. Specification Status and Open Design Questions”; “15.1 Deferred Language Areas”; “15.2 Other Major Underspecified Areas”; “15.3 Open Design Questions” | `docs/language/STATUS.md` plus focused open-question sections only where needed. |
| “15.4 Resolved Decisions” | Verify and distribute each retained decision to its semantic owner; discard chronological duplication. |

## Implemented grammar headings

| Source heading | Intended authoritative destination |
|---|---|
| “Implemented Skald Grammar”; “Lexical structure”; “Literals”; “Compilation unit and declarations”; “Statements and blocks”; “Expressions”; “Recovery and nesting” | Exact syntax moves to `docs/language/GRAMMAR.md`; semantic restrictions link to focused language owners. |
| “Primitive semantics” | Syntax remains in `GRAMMAR.md`; type and operator meaning moves to `TYPES_AND_VALUES.md`. |
| “Inline classes”; “Implemented extension: class-typed inline fields” | Syntax remains in `GRAMMAR.md`; object and initialization meaning moves to `CLASSES_AND_LIFECYCLE.md`. |
| “Restricted extension: deterministic destruction”; “Implemented extension: object value semantics” | Syntax remains in `GRAMMAR.md`; lifetime, copy, and value behavior moves to `CLASSES_AND_LIFECYCLE.md`. |
| “Not yet implemented” | `docs/language/STATUS.md`; retain only syntax constraints that are actually frozen. |

## Compiler and workflow headings

| Source heading or headings | Intended authoritative destination |
|---|---|
| `docs/REPO_STRUCTURE.md` — “Repository Structure and Compiler Architecture”; “Design principles”; “Relationship to Niflheim” | `docs/compiler/README.md`. |
| “Top-level layout”; “crates/skac/”; “crates/skald-compiler/”; “Compiler crate API policy”; “Other directories” | Durable roles move to `docs/compiler/README.md`; exact file inventories are removed. |
| “runtime/” | `docs/compiler/RUNTIME_ABI.md`. |
| “Compiler pipeline”; “Source and diagnostics”; “Syntax”; “Resolution and identities”; “Typed HIR”; “MIR” | `docs/compiler/PHASES_AND_IR.md`. |
| “x86-64 System V backend” | `docs/compiler/BACKEND.md`. |
| “Driver and artifacts” | `docs/compiler/DRIVER_AND_ARTIFACTS.md`. |
| “Testing” | `docs/development/TESTING.md`. |
| “Extension policy” | `docs/compiler/README.md`, with feature ordering linked to roadmaps. |
| `docs/NEXT_SLICE_BOUNDARIES.md` — “Future Development Boundaries”; “Stable compiler responsibilities”; “Rules for extending the language”; “Compiler evolution” | Stable policy moves to `docs/compiler/README.md`; maturity and ordering link to status and roadmaps. |
| “Object-model sequence”; “Other planned language work” | `docs/language/STATUS.md` and focused active roadmaps. |
| `docs/DEBUGGING.md` — “Compiler Debugging Artifacts” | `docs/development/DEBUGGING.md`. |
| `samples/README.md` — “Samples” | Retain locally as sample-catalog mechanics. |
| `scripts/README.md` — “Scripts” | Retain locally as script ownership policy. |
| `std/README.md` — “Standard Library” | Retain locally as current status. |
| `tests/README.md` — “Tests” | Retain locally as the test-tree entry point; general policy moves to `docs/development/TESTING.md`. |
| `tests/compiler/README.md` — “Compiler Tests”; `tests/compiler/robustness/README.md` — “Compiler Robustness Tests” | Retain concise corpus mechanics and link the general testing authority. |
| `tests/golden/README.md` — “Golden Tests” | Retain golden fixture and expectation mechanics. |
| `tests/runtime/README.md` — “Runtime Tests” | Retain harness mechanics; ABI statements move to `docs/compiler/RUNTIME_ABI.md`. |

## Incoming links to superseded authorities

These repository links must be redirected before DOC16 removes their targets.
Line numbers are intentionally omitted so ordinary prose editing does not make
this ledger stale.

| Current target | Incoming Markdown sources |
|---|---|
| `docs/SKALD_DRAFT_SPEC.md` and its anchors | `README.md`, `docs/README.md`, `docs/REPO_STRUCTURE.md`, `grammar/README.md`, and `docs/archive/README.md`. |
| `grammar/README.md` compatibility path | Historical roadmap prose and external links may still name it; all living repository authorities now link directly to `docs/language/GRAMMAR.md`. |
| `docs/REPO_STRUCTURE.md` | `README.md`, `docs/README.md`, `docs/compiler/README.md`, and `docs/archive/README.md` while the test replacement remains pending. |
| `docs/NEXT_SLICE_BOUNDARIES.md` | `README.md`, `docs/README.md`, and `docs/archive/README.md`. |
| `docs/DEBUGGING.md` | `README.md` and `docs/README.md`. |

Archived roadmap prose also names several legacy paths in code formatting.
Those are historical references rather than links; DOC16 preserves the prose
and repairs only navigation needed to reach living authorities.

## Source-code documentation references

| Source | Current reference | Planned destination |
|---|---|---|
| `crates/skald-compiler/src/lib.rs` crate documentation | `docs/compiler/README.md` and `docs/compiler/PHASES_AND_IR.md` | Focused destinations established. |
| `crates/skald-compiler/src/lexer/mod.rs` module documentation | `docs/language/GRAMMAR.md` | Focused destination established. |
| `crates/skald-compiler/src/backend/mod.rs` and `backend/x86_64_sysv/mod.rs` module documentation | `docs/compiler/BACKEND.md` | Focused destination established. |
| `runtime/include/skald_runtime.h` and the compiler runtime-marker comment | `docs/compiler/RUNTIME_ABI.md` | Focused destination established. |
| `crates/skald-compiler/src/driver/mod.rs` module documentation | `docs/compiler/DRIVER_AND_ARTIFACTS.md` | Focused destination established. |

## Verification boundary

`make docs-check` validates all repository Markdown relative file links, local
heading anchors, and required entries in `docs/README.md`,
`docs/roadmaps/README.md`, and `docs/archive/README.md`. The checker provides a
dynamic safety net; this inventory records semantic disposition and references
that must change when legacy authorities are finally removed.
