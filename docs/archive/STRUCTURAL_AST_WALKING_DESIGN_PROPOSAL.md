# Structural AST Walking Design Proposal

Status: frozen decision record. Accepted on 2026-09-13; implementation is
tracked by the
[Structural AST Walking Roadmap](../roadmaps/STRUCTURAL_AST_WALKING_ROADMAP.md).

This proposal gives source-shaped syntax traversal one narrow owner. It
centralizes the structural child relationships that compiler-dependency
collection, parser depth measurement, and generic-specialization request
discovery currently repeat, while keeping dependency meaning, depth policy,
type closing, scopes, diagnostics, and semantic transformation with their
existing phases.

The change is internal to the compiler. It does not add source syntax, change
evaluation order, introduce a common framework for later IRs, or make syntax
responsible for resolution.

## Intended outcome

- Define every traversable syntax node and its children once under `syntax`.
- Give that structural order an explicit source-order contract rather than
  relying on the field order of individual AST structs.
- Offer enter/leave events and per-node continue/prune control without putting
  consumer state or semantics into the walker.
- Implement the traversal iteratively so sharing it does not weaken the
  expression-depth and stack-safety guarantees delivered by A01.
- Migrate compiler-dependency collection and expression-depth measurement
  first, then migrate generic-specialization source scanning after its
  identity-sensitive ordering is characterized.
- Make a new child-bearing statement or expression require one structural
  walker change and focused traversal-test update. Consumers should change
  only when the new form has meaning for them.
- Leave semantic visitors explicit when traversal and meaning cannot be
  separated cleanly.

## Current architecture and evidence

Three current owners repeat complementary portions of the source tree:

| Consumer | Current traversal | Meaning that must remain local |
| --- | --- | --- |
| [`module::graph::compiler_dependencies`](../../crates/skald-compiler/src/module/graph/compiler_dependencies.rs) | Recursively walks declarations, class members, blocks, and statements to find direct range sources; string-literal and general-iteration evidence comes from lexer tokens. | Mapping accepted syntax to typed canonical-module dependencies and preserving evidence spans. |
| [`syntax::parser::expression_depth`](../../crates/skald-compiler/src/syntax/parser/expression_depth.rs) | Iteratively enumerates every expression child while carrying actual-expression and logical-expression depth. | Parser resource limits, depth arithmetic, diagnostic selection, and recovery. |
| [`resolve::specialization::requests::source_request_scanner`](../../crates/skald-compiler/src/resolve/resolver/program/specialization/requests/source_request_scanner.rs) | Recursively walks declarations, members, types, blocks, statements, and expressions to close explicit generic applications. | Template pruning, module lookup, type closing, request identity allocation, provenance, and diagnostics. |

The three files currently contain about 545 lines, but line count is secondary.
The maintenance risk is that statement and expression variants are listed in
several places. A new syntax form can compile after one consumer is updated
while another exhaustive match either treats it as a leaf or acquires a new
local traversal with subtly different ordering.

The AST already has the right ownership boundary. [`syntax::ast`](../../crates/skald-compiler/src/syntax/ast.rs)
owns source-shaped nodes and spans without resolved identities or inferred
types. [`syntax::mod`](../../crates/skald-compiler/src/syntax/mod.rs) is a
facade over that representation, parsing, and dumping. Structural child
relationships belong beside these nodes; dependency kinds and specialization
requests do not.

A01 established an important constraint. Accepted expression trees are
bounded, and expression-depth measurement is iterative. Moving that code onto
a recursive visitor would trade a demonstrated robustness fix for tidier
source. The shared traversal must therefore own an explicit heap work stack.

The specialization scanner calls `SyntaxTypeCloser`, which recursively closes
one complete type occurrence and may allocate class or interface identities.
Its order is observable through stable identities, provenance, diagnostics,
and resolved dumps. Inspection found one exception to its stated source-order
contract: a local declaration currently scans its initializer before its
earlier type annotation. That order is residue from the removed syntax-level
range inference. It must be corrected and characterized separately from the
mechanical walker migration.

Other AST matches are not automatically duplication:

- the AST dumper owns punctuation, indentation, labels, and a rendering
  protocol rather than a generic traversal action;
- ordinary resolution, generic-template body analysis, and type checking
  interleave scope mutation, diagnostics, short-circuiting, and construction
  of new phase products;
- parameter-dependency queries fold semantic facts and stop as soon as a
  dependent child is found; and
- resolved, HIR, and MIR walks traverse different representations with
  different invariants.

Those consumers provide evidence for future local helpers only if two of them
later repeat the same structural responsibility. They do not justify widening
A13 into a compiler-wide visitor hierarchy.

The sibling Niflheim repository has semantic and lowering refactor records but
no structural walker whose contracts match Skald's current source AST,
resource limits, direct range syntax, or specialization identity ownership.
Skald's current syntax and phase contracts remain authoritative.

## Scope and invariants

- The walker is owned by `syntax` and is visible only inside the compiler
  crate. It is not part of the workspace-facing syntax API.
- Traversal borrows the AST immutably. It cannot mutate nodes, allocate
  identities, append diagnostics, or construct a later-phase product.
- Child order follows source spelling. Vector children retain vector order;
  optional children appear only when present.
- Enter and leave events are properly nested. A pruned node still receives its
  leave event, allowing consumer-owned stacks to remain balanced.
- Pruning omits only descendants of the selected node. It does not skip later
  siblings or suppress the node itself.
- Type and named-type occurrences are yielded as opaque leaves. Their recursive
  interpretation remains with the type parser, resolver, and
  `SyntaxTypeCloser`; the structural walker does not duplicate type closing.
- Expression walking remains iterative and uses memory proportional to pending
  structural work. No consumer migration may introduce recursive expression
  descent through callbacks.
- Compiler dependencies retain their existing typed kinds, canonical module
  mapping, and exact source spans. Token-derived string and `for` evidence
  remains token-derived.
- Depth measurement retains root depth one, the existing logical-depth
  definition, all accepted/rejected boundaries, `PAR005`, and declaration
  recovery.
- Specialization discovery retains module order, source order, template
  pruning, type-closing behavior, request deduplication, provenance spans,
  diagnostics, and deterministic identity allocation after the local-order
  correction is frozen.
- AST, resolved, HIR, MIR, diagnostic, report, and native outputs remain
  unchanged by mechanical migrations.
- A new statement or expression variant must make the syntax-owned child match
  non-exhaustive. Consumer matches may remain unchanged when they subscribe
  only to node categories or variants relevant to their meaning.

## Non-goals

- No generic visitor framework shared by syntax, resolved IR, HIR, and MIR.
- No mutable AST visitor, rewriting framework, fold, arena conversion, or AST
  parent pointers.
- No transfer of binding scopes, declaration filtering, dependency policy,
  depth arithmetic, type closing, diagnostics, or identity allocation into
  `syntax`.
- No replacement of parser construction logic or syntax dumping.
- No migration of ordinary resolution, generic-template body resolution,
  type checking, HIR lowering, or semantic range discovery merely because
  those owners contain AST matches.
- No change to lexer-token dependency evidence for string literals and general
  iteration.
- No public plugin or extension API for registering visitors.
- No performance cache. Traversal is a request-local borrowed operation over
  an existing AST.

## Selected design

### A private syntax facade owns traversal

Add a private recursive module under `syntax`:

```text
syntax/walk/
├── mod.rs
├── traversal.rs
└── tests.rs
```

`syntax::mod` selectively re-exports the small surface as `pub(crate)` so later
phases use the syntax facade without making traversal public to workspace
consumers. `walk::mod` contains module documentation and the node/control
vocabulary. `traversal` owns the work stack and exhaustive child enumeration.
Tests stay with this private structural owner.

The exact Rust spelling may adjust during implementation, but the conceptual
surface is:

```rust
#[derive(Clone, Copy)]
pub(crate) enum SyntaxNode<'ast> {
    CompilationUnit(&'ast CompilationUnit),
    Import(&'ast ImportDeclaration),
    Declaration(&'ast TopLevelDeclaration),
    ClassMember(&'ast ClassMember),
    Block(&'ast Block),
    Statement(&'ast Statement),
    Expression(&'ast Expression),
    Type(&'ast TypeSyntax),
    NamedType(&'ast NamedTypeSyntax),
}

#[derive(Clone, Copy)]
pub(crate) enum WalkControl {
    Continue,
    Prune,
}

pub(crate) trait SyntaxVisitor<'ast> {
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl;
    fn leave(&mut self, node: SyntaxNode<'ast>);
}

pub(crate) fn walk<'ast>(
    root: impl Into<SyntaxNode<'ast>>,
    visitor: &mut impl SyntaxVisitor<'ast>,
);
```

This is one event interface rather than a trait method for every AST variant.
Consumers match only node categories and variants with meaning for them. The
exhaustive expression and statement matches remain in the syntax-owned child
enumerator, where adding a new variant produces a compiler error.

Convenience root functions are acceptable when they improve call sites, but
they must delegate to the same engine. There should not be separate recursive
and iterative definitions of expression children.

### Events separate structure from meaning

The engine emits `enter(node)`, then either the node's children or none,
followed by `leave(node)`. `WalkControl::Prune` returned from `enter` suppresses
the children. The engine ignores control on leave by making `leave` return
unit.

This gives consumers enough control without callback-driven recursion:

- compiler-dependency collection continues through declarations, members,
  blocks, and statements, records a direct range operator on the enclosing
  `ForIn` statement, prunes expression subtrees, and ignores type leaves;
- expression-depth measurement starts at one expression, updates its counters
  on expression enter/leave, and ignores opaque type occurrences;
- specialization discovery prunes generic class and interface declarations,
  and closes each yielded opaque type occurrence exactly once; and
- callable or block stacks remain consumer state, balanced by enter/leave
  events when a future structural consumer actually needs them.

The walker never accepts a closure that recursively invokes `walk` for a
child. That would make stack behavior depend on consumer code and restore the
omission risk this design removes.

### Child order is a named contract

Structural children are emitted in source spelling order. The important
composite cases are:

| Node | Child order |
| --- | --- |
| Compilation unit | imports, then declarations, each in stored order |
| Function-like declaration/member | parameter types, result type when present, then body |
| Class | direct base, implemented interfaces, where-clause interfaces, then members |
| Interface | where-clause interfaces, then each requirement's parameter and result types |
| Static field | declared type, then initializer |
| Local declaration | declared type, then initializer |
| Conditional | `if` condition/body, each `elif` condition/body, then optional `else` body |
| `while` | condition, then body |
| `for-in` | optional annotation, iterable or lower/upper range source, then body |
| Assignment | place components in source/evaluation order, then value |
| Prefix cast or construction | written target, then operand or arguments |
| Infix, logical, call, and projection expressions | receiver/left/callee first, then remaining expressions from left to right |
| Slice | receiver, optional start, then optional end |

Imports are leaf events because their names are handled by module import
resolution. Types and named types are leaf events because one consumer-owned
type operation already traverses their internal shape atomically. The walker
may gain type-child traversal only after two consumers need the same operation;
it must not be added speculatively.

The work stack pushes children in reverse so emitted events retain the table's
forward order. Tests assert the emitted order rather than depending on that
implementation detail.

### The local-declaration order discrepancy is corrected explicitly

The current specialization scanner visits a local initializer before its
declared type even though the source grammar places the type first. This order
arose when the scanner also inferred range requests and had to keep a new
binding out of its own initializer; the semantic range pass now owns that
work.

Before migrating specialization discovery, add a focused test with distinct
closed generic applications in the annotation and initializer. Change the
scanner to visit the annotation first, update only identity/dump expectations
that demonstrably follow from that correction, and verify diagnostics and
native behavior. This must be a separate reviewed change from replacing the
scanner's recursion with `syntax::walk`.

After that correction, the walker trace and specialization characterization
must agree. Future migrations preserve that order exactly. Sorting all
specialization requests after discovery is rejected because recursive
specialization activation and identity assignment are intentionally part of
the coordinator's deterministic request protocol.

### Consumer policies stay in their owners

Compiler-dependency collection keeps the `BTreeMap` and
`CompilerDependencyKind` mapping under `module`. The shared statement event
exposes the existing `ForInSource::Range` field to that consumer; the walker
does not know that its operator requests `std::range`. Token iteration remains
beside dependency collection because it preserves exact literal and keyword
evidence without walking expression payloads.

Expression-depth measurement keeps its `Depths` product and counters under the
parser. Entering an expression increments actual depth; entering a logical
expression increments logical depth; the paired leave restores both. Opaque
type nodes do not affect either measure. The implementation must compare this
definition against the current accepted/rejected boundary tests before the old
match is removed.

Specialization discovery keeps `SyntaxTypeCloser` and the coordinator under
resolution. The visitor decides whether a declaration is an unrequested
generic template and returns `Prune`. On a type or named-type event it performs
the existing close operation; that occurrence has no walker-owned descendants.
No resolver type, lookup table, diagnostic, or specialization key crosses into
`syntax::walk`.

## Characterization and migration strategy

Implementation should proceed in behavior-preserving stages after this design
is accepted:

1. Add traversal characterization around the current owners. Build a parsed
   source fixture containing every current child-bearing statement and
   expression family, including both `for-in` sources, every array constructor
   argument form, ordinary/copy call arguments, optional-box initializers, and
   index/slice projections. Record exact node/event order and spans.
2. Add the private iterative walker and tests for enter/leave nesting,
   left-to-right order, pruning, optional children, and continuation after a
   pruned subtree. Do not migrate a consumer until these tests pass.
3. Migrate expression-depth measurement and compiler-dependency range
   collection. These are the two non-semantic walkers and establish that the
   API supports both expression-root and compilation-unit traversal without
   moving their policies.
4. Correct and freeze the local type-before-initializer specialization order in
   its own focused change. Capture class/interface identities, provenance
   spans, diagnostics, and dumps before and after so the only accepted delta is
   the intended source-order correction.
5. Migrate specialization source discovery onto the walker. Preserve generic
   template pruning and every other request order. Remove the old recursive
   statement/expression plumbing only after equivalent focused and
   process-determinism observations pass.
6. Audit remaining AST matches by responsibility. Adopt the walker only where
   a consumer merely observes structure. Record larger semantic candidates as
   discoveries rather than turning the walker into a transformation framework.

Each stage should leave the compiler buildable and the full quality gate green.
A roadmap should split these stages into reviewable PRs rather than combine the
new traversal contract, identity-order correction, and all migrations.

## Test and validation design

### Walker contract tests

Private syntax tests should prove:

- every current child-bearing statement and expression reaches its children;
- leaf expressions emit no invented descendants;
- ordinary vectors, call arguments, element lists, conditional arms, range
  endpoints, and slice bounds preserve exact source order;
- enter/leave events are balanced and nested;
- pruning an expression, block, member, or generic declaration omits only its
  descendants;
- traversal resumes with the next sibling after pruning; and
- every emitted type, named type, expression, and statement reference retains
  its original source span, as does a range operator reached through its
  enclosing statement.

The all-shapes fixture should use parsed source so the test covers real parser
products. Small hand-built nodes are appropriate only for a structural corner
that valid source cannot isolate. Tests should compare compact semantic event
labels and source slices, not debug output that changes when unrelated AST
fields are added.

### Consumer regressions

- Existing syntax nesting tests retain the precise expression and logical
  depth boundaries. The process-isolated expression-depth robustness test
  retains its external watchdog and hostile flat/postfix inputs.
- Module graph tests add nested range sources across functions, class members,
  generic class bodies, conditionals, loops, and explicit blocks. They assert
  the exact ordered `..` spans while string and general-iteration token
  evidence remains unchanged.
- Specialization tests place different generic applications in declaration
  types, class headers, static initializers, local annotations and
  initializers, casts, allocations, arrays, calls, conditionals, loops, and
  projections. They assert stable keys, identities, origins, pruning of
  unrequested templates, diagnostics, and resolved dumps.
- Pipeline determinism retains independent-process byte comparisons for AST,
  resolved products, diagnostics, and relevant phase checkpoints.
- Representative golden programs retain native output and ownership traces;
  no new golden-only syntax corpus should duplicate the private all-shapes
  walker fixture.

Every implementation PR runs its focused owner tests, `cargo fmt --all --
--check`, and `make check`. Changes to Rust implementation or visibility also
run `make msrv-check` for Rust 1.82. The final migration should additionally
run the release expression-depth robustness case documented in the testing
guide.

## Rejected alternatives

### Keep the duplicated matches

Exhaustive matches catch newly added enum variants only in files that match
without a wildcard, and they do not ensure equal child order. The current
three-way review burden has already retained a stale local-order rule. Keeping
it makes each syntax extension more error-prone.

### Use a recursive default visitor

A recursive visitor is conventional and concise, but it would make stack use
depend on structural depth and callback behavior. A01 specifically removed
that risk from depth validation. One iterative engine gives all structural
consumers the same bounded call stack.

### Expose child slices from AST nodes

Expressions and statements have heterogeneous children: expressions, blocks,
types, named types, and optional/vector forms. Returning boxed iterators or
allocated child vectors would add allocation and lifetime complexity at every
node. Encoding children in the private work-stack builder keeps that
complexity in one place and exposes only borrowed node events.

### Put semantic hooks on every node kind

A large trait with `visit_binary`, `visit_call`, `visit_local`, and default
methods recreates the whole AST as an inheritance-like API. Consumers would
override many hooks, and traversal order could again become split between
defaults and overrides. One node event plus explicit prune control is smaller
and makes consumer meaning visible in one match.

### Migrate all AST consumers

Resolution and type checking often need to process one child, mutate scope,
use the result, then decide whether or how to process another child. Forcing
those transformations through observation events would hide control flow and
make diagnostics harder to reason about. A13 targets repeated structural
observation only.

### Sort observations by span after traversal

Span sorting appears to guarantee source order but loses parent/child
structure, treats equal or covering spans ambiguously, and cannot represent
enter/leave scopes. It would also detach specialization activation from the
coordinator's deliberate discovery order. Explicit child order is the durable
contract.

### Generate walkers from enum declarations

Rust derives cannot infer semantic child order or decide which fields are
structural children. A procedural macro or build script would add a second
language for a small closed AST and make review harder. An exhaustive Rust
match is clearer.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Identity or diagnostic order changes during specialization migration | Correct the known local-order discrepancy separately, then compare exact resolved dumps, origins, and process-determinism outputs before removing the old scanner. |
| The walker becomes a semantic dependency hub | Keep it under `syntax`, borrow syntax only, and expose no generic context, diagnostic, identity, or output type. |
| A consumer accidentally descends into an opaque type twice | Type events have no structural descendants; specialization closes each complete occurrence once. Focused nested-generic tests assert one canonical request and complete origins. |
| Pruning unbalances consumer scopes | Emit leave for every entered node, including pruned nodes, and test nested pruning with a stack-counting visitor. |
| Iteration order reverses because of the LIFO work stack | Specify forward child order, push in reverse internally, and assert exact event/source-slice traces. |
| The shared walk adds unnecessary work to narrow consumers | Allow pruning at expression/type/member boundaries and compare focused measurements only if a meaningful compile-time regression appears. Avoid caching or parallelism without evidence. |
| New AST variants still bypass consumers | Keep statement/expression child matches exhaustive and require the all-shapes fixture to name each child-bearing family. Consumer-specific semantic matches remain separately exhaustive where meaning requires it. |

## Decision summary

| Question | Decision |
| --- | --- |
| Who owns structural AST traversal? | The private `syntax::walk` facade. |
| What is shared? | Immutable node relationships, source order, enter/leave events, and continue/prune control. |
| What remains phase-owned? | Dependency meaning, depth policy, scopes, type closing, identities, diagnostics, and transformed products. |
| Is traversal recursive? | No. One explicit work stack drives every shared walk. |
| Are types recursively walked? | No. Type and named-type occurrences are opaque leaves until repeated structural demand justifies more. |
| Which consumers migrate first? | Expression-depth measurement and compiler-dependency range collection. |
| Does specialization discovery migrate? | Yes, after exact ordering is characterized and the known local-order discrepancy is corrected separately. |
| Do ordinary resolver/type-checker transformations migrate? | No. Their traversal is inseparable from semantic state and output construction. |
| Does this change public compiler APIs or language behavior? | No public API or language change. One internal specialization identity-order correction restores the already stated source-order contract and receives explicit regression coverage. |
| What happens if the event interface obscures a consumer? | Keep that consumer explicit; shared traversal is adopted only where it reduces repeated structural responsibility. |

## Acceptance criteria for promotion

Promote this proposal to a frozen decision and create an implementation
roadmap only if review accepts all of the following:

- syntax owns a crate-private iterative event walker;
- source-order children, balanced enter/leave events, and pruning semantics are
  the complete shared contract;
- type syntax remains opaque and semantic policies remain in their phases;
- the two non-semantic consumers migrate before specialization discovery;
- the local annotation/initializer order correction is isolated from the
  scanner migration;
- ordinary semantic transforms and later IRs remain outside A13; and
- the roadmap separates contract, non-semantic migrations, ordering
  correction, semantic scanner migration, and final audit into reviewable
  changes with the validation described above.
