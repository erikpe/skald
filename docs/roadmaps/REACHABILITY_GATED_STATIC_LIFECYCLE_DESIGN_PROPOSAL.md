# Reachability-Gated Static Lifecycle Design Proposal

Status: proposed; RSL1 through RSL12 await confirmation. This document does
not describe current language behavior.

The current [static-field language contract](../language/STATIC_FIELDS.md)
activates every declared static field in the loaded closed program before the
selected Skald entry function and destroys every such field after normal
return. The implemented
[static-lifecycle certificate](../archive/STATIC_LIFECYCLE_CERTIFICATE_DESIGN_PROPOSAL.md)
freezes that complete activation order before optimization, while the later
[whole-world reachability boundary](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
currently treats every coordinator activation and shutdown operation as an
execution root.

This proposal changes the language contract so only statics required by the
canonical entry-rooted execution closure acquire a runtime lifetime. The
selected subset still initializes eagerly before entry, remains live for the
entire entry invocation, and shuts down in exact reverse order after normal
return. Module loading, declaration checking, type checking, and preliminary
MIR remain whole-world and complete; an unused declaration simply does not
cause runtime activation.

The change is intentionally semantic rather than an optional optimization.
The same activation set, startup effects, shutdown effects, diagnostics, and
native behavior must apply under `none`, `default`, pass exclusions, and every
future target. Skald's permanent whole-world compilation makes the set finite,
and single-threaded generated execution means no runtime once-state,
synchronization, or concurrent activation protocol is required.

Explicit `eager static`, module initialization, registration blocks, lazy
first-access initialization, and a source annotation that forces retention are
deferred. A program that wants an initializer to run in this first contract
must contain an ordinary statically reachable access to its field.

## Motivation

Source loading is deliberately broader than executable need. For example,
`std::str::Str` imports `std::str::parse_f64` because `Str.to_f64` can call it.
A program that uses integer string formatting but never reaches `to_f64`
nevertheless loads, resolves, type-checks, lowers, initializes, and later
destroys the large `_EiselPowers._words` table under the current contract.
Whole-world callable reachability can remove the unused parser bodies, but it
cannot remove the table because every static activation is already a semantic
root.

The current behavior has three costs:

- an unused library feature may allocate, fail, print, mutate another static,
  or run user destruction solely because its module was transitively imported;
- all static initializer and shutdown work becomes a root that retains its
  transitive callable, lifecycle, literal, and runtime dependencies; and
- large table-driven library features cannot become pay-for-use without
  manually splitting their types or modules.

The proposed behavior makes a static lifetime follow executable need while
preserving eager-before-entry access, deterministic ordering, exact reverse
shutdown, and all existing ownership guarantees for fields that are active.

## Goals

- Define an exact source-visible distinction between a declared static field
  and an active static field.
- Activate fields at field granularity from one deterministic, mandatory,
  pre-optimization whole-program analysis.
- Include static accesses reached through calls, dynamic dispatch, function
  values, copy/destruction, ownership, optionals, arrays, and already-active
  static initialization or shutdown.
- Keep every active field live before entry begins; do not add runtime lazy
  initialization or per-access guards.
- Preserve the current dependency order and exact-reverse normal-return
  shutdown semantics over the active subset.
- Keep module loading, name resolution, type checking, declaration identities,
  source diagnostics, and preliminary initializer bodies complete.
- Bind the active set to the static-lifecycle certificate so optimization
  cannot silently narrow or expand it.
- Let final MIR, backend planning, and emitted artifacts omit inactive static
  initializer, cleanup, storage, and dependency work where their respective
  representation permits it.
- Provide deterministic explanations and counts so a developer can answer why
  a field did or did not activate.

## Non-goals

- No new source syntax, `eager` modifier, module initializer, registration
  hook, or force-retention attribute.
- No initialization on first runtime access and no initialized-state flag.
- No top-level globals, interface statics, external statics, thread-local
  storage, atomics, reflection, or dynamic loading.
- No purity/effect proof that deletes an otherwise active initializer.
- No post-optimization replanning or profile-specific static lifetime.
- No module, class, declaration, field-ID, type-table, or source-span
  compaction.
- No requirement to remove every inactive target-private slot or metadata
  object in the first implementation when existing artifact retention can
  remove it safely.
- No weakening of checks inside inactive initializer expressions: parsing,
  resolution, typing, ownership, and preliminary MIR validity remain complete.
- No change to abrupt termination. Panic, signals, initializer failure, and
  foreign process termination remain non-unwinding for static cleanup.

## Terminology

**Declared static** means a source declaration with a canonical
`StaticFieldId`. It participates in namespace, inheritance, privacy, typing,
generic specialization, HIR, and preliminary MIR whether or not it is active.

**Activation access** means an ordinary possible read, write, replace, borrow,
or other use of a static place from activation-reachable execution. The
lifecycle-owned unpublished destination write inside a field's own initializer
is not an activation access; otherwise every explicit declaration would root
itself.

**Active static** means a declared field in the exact least fixed point defined
below. It owns a runtime lifetime, appears in the static-lifecycle plan and
certificate, and participates in startup and shutdown.

**Inactive static** means a declared field outside that fixed point. Its
initializer and eventual-value cleanup do not execute, and its storage is not
source-observable. Its declaration and initializer remain checked.

**Activation-reachable execution** is the canonical execution-node closure
computed from verified preliminary MIR before selectable optimization. It is
not actual path coverage from one run and is not a compiler-chosen
conservative approximation that may vary by profile.

## Proposed language contract

### Declaration does not imply activation

Declaring, importing, specializing, or type-checking a static field does not
give it a runtime lifetime. Using a class as a type, constructing an instance,
calling an unrelated member, retaining class metadata, or loading the module
that owns the class also does not activate its statics.

An inactive explicit initializer is not evaluated. It therefore cannot
allocate, panic, perform I/O, mutate another static, publish an owner, or run
full-expression cleanup. Its declared eventual value is never live and is not
destroyed at shutdown. An initializer-free inactive field similarly has no
source-observable live zero-default interval.

This is a deliberate compatibility change. An otherwise unused static
initializer is no longer an implicit registration or module-initialization
mechanism. Until explicit eager or module initialization is designed, such
work must be called or accessed through ordinary reachable code.

### Exact activation triggers

The semantic trigger is an activation access found in the canonical closure,
not merely a textual reference anywhere in the program.

| Situation | Activates the field? | Reason |
|---|---:|---|
| Reachable read of `Class.field` | Yes | The value may be observed |
| Reachable assignment or replacement | Yes | Storage must be live before the operation |
| Reachable `ref` or `mut ref` argument sourced from the field | Yes | Borrowing is a static-place access |
| Access from a reachable static method | Yes | Static methods are ordinary execution nodes |
| Access reached transitively through calls or lifecycle work | Yes | The closure follows canonical execution dependencies |
| Access from an already-active field's initializer | Yes | The target must activate before the dependent field |
| Access from an already-active field's eventual destruction | Yes | The target must remain live during dependent shutdown |
| Field declaration or initializer expression existing | No | The initializer does not execute until the field is active |
| Field's lifecycle-owned initializer destination | No | Publication cannot bootstrap its own activation |
| Importing its module | No | Imports select declarations, not runtime initialization |
| Using or instantiating its declaring class | No | Activation is per field, not per class |
| Access only from an activation-unreachable callable | No | That callable cannot participate in the canonical execution closure |
| Type, layout, conformance, or dispatch metadata mentioning the class | No | Metadata reachability is not a static-place access |

All current static stored types, mutable/final rules, privacy, and access
semantics remain unchanged after activation. Finality does not itself activate
a final static.

### Canonical control-flow and possible-target rule

Activation is computed from verified preliminary MIR before selectable MIR
passes. Every structurally present block of an activation-reachable execution
node is scanned, even when an earlier constant condition makes the block
unreachable at runtime. Consequently, this activates `Cache.value`:

```ska
fn main() -> i64 {
    if (false) {
        return Cache.value;
    }
    return 0;
}
```

This rule is intentionally conservative and stable. A future CFG pass may
remove the branch from final MIR, but it may not retroactively suppress the
initializer or destructor effects selected by the source program's canonical
activation analysis.

Direct and static calls follow their exact targets. Virtual calls use the full
verified virtual family, interface calls use every verified implementation of
the requirement, and implicit copy, assignment, destruction, optional,
shared-owner, and array operations follow their canonical lifecycle plans.
Function-value address formations and indirect calls use the same exact-type,
closed-world coupled target rule as target-independent reachability. Forming a
callable address in activation-reachable execution makes that callable part of
the activation closure even if no surviving indirect call invokes it.

These target-expansion rules are part of the language contract for activation.
Later rapid-type analysis, points-to analysis, devirtualization, CFG cleanup,
or address-use elimination may optimize code but may not change which static
side effects occur. Narrowing activation in a future language version would
require an explicit contract revision.

### Transitive activation and ordering

Activating a field adds the following work to the closure:

- its explicit initializer execution node, when present;
- the canonical lifecycle nodes needed to destroy its current eventual value;
- its required static storage runtime entity; and
- every execution and static-access dependency of that initializer and
  destruction work.

New activation accesses discovered from that work activate more fields until
the set stops growing. If active field `A`'s initialization or destruction may
access `B`, then `B` is active and remains the prerequisite of `A` under the
existing lifetime graph. An inactive field's initializer does not activate its
own dependencies because the initializer never enters the closure.

Planning topologically orders only the active fields, using the current stable
`StaticFieldId` tie break. Every active field is live before entry starts.
Shutdown visits exactly those fields in exact reverse activation order after
normal entry return. Existing publication, self-access, ownership,
replacement, destruction, and non-unwinding rules apply unchanged to that
active subset.

### Source examples

An unused side-effecting declaration is inactive:

```ska
class Dormant {
    static value: i64 = announce_and_return();
    init() {}
}

fn main() -> i64 {
    return 0;
}
```

`announce_and_return` is still resolved, type-checked, and lowered to verified
preliminary MIR, but it does not execute.

An ordinary reachable access activates the field before entry:

```ska
fn main() -> i64 {
    return Dormant.value;
}
```

Activation follows initializer dependencies:

```ska
class Values {
    static base: i64 = make_base();
    static derived: i64 = Values.base + 1;
    init() {}
}

fn main() -> i64 {
    return Values.derived;
}
```

Both fields activate, `base` precedes `derived`, and shutdown uses the reverse
order. If neither field is activation-reachable, neither initializer executes.

## Compiler phase placement

The semantic choice must occur after all executable dependencies and static
accesses are explicit, but before static-lifetime planning treats fields as
roots:

```text
whole-world resolution and type checking
                │
                ▼
definition-complete preliminary MIR
                │
                ▼
preliminary MIR verification
                │
                ▼
shared execution/static-effect extraction
                │
                ▼
mandatory entry-rooted static activation closure   ← new semantic boundary
                │
                ▼
active-subset lifecycle graph, diagnostics, order, and certificate
                │
                ▼
active-subset coordinator synthesis and verification
                │
                ▼
selectable final-MIR optimization pipeline
                │
                ▼
backend lowering and target-private artifact retention
```

Running activation after the selectable pipeline is forbidden. It would make
initializer effects, shutdown effects, cycle diagnostics, and potentially
program acceptance depend on `none`, `default`, pass order, or target.

## Activation analysis

### Inputs and ownership

The analysis consumes only verified preliminary MIR plus the shared exhaustive
execution-dependency and static-effect extraction boundaries. It must not
reconstruct call meaning from source syntax or own a second target resolver.

The natural implementation owner is a focused activation submodule under
`passes::static_lifecycle`. It owns field-activation policy, the coupled
fixed-point solver, deterministic reasons, and planning metrics. The existing
`passes::reachability` facade continues to own neutral execution nodes,
dependency targets, lifecycle expansion, and possible-target selection.
Neither the final pruning pass nor the backend owns activation policy.

Static-effect inference may continue computing summaries for all preliminary
execution nodes. Those complete summaries are useful for checking inactive
initializer bodies and for quickly finding static accesses, but only the
entry-rooted activation solver decides which summaries become lifecycle roots.

### Deterministic least fixed point

Conceptually, the solver maintains two sorted worklists:

```text
reachable execution nodes := { selected entry }
active static fields       := {}

while either worklist changes:
    for each newly reachable execution node:
        follow canonical execution dependencies
        activate every ordinary static-access target

    for each newly active static field:
        require its storage entity
        reach its explicit initializer, if any
        reach its eventual-value destruction nodes
```

The lifecycle-owned unpublished write to the newly active field is excluded
from activation dependency discovery but remains in ordinary initializer
verification. All other accesses, including after-publication self-access,
retain their current lifetime meaning and diagnostics.

Sorted identities and existing canonical dependency keys determine worklist,
edge, and first-witness order. Compiler implementation parallelism may compute
independent extraction data, but it may not change the active set, explanation,
diagnostic, dump, or plan.

### Analysis result and explanations

The planning-only result should provide borrowed deterministic queries for:

- active fields and inactive declared fields;
- the first activation access and root-to-access witness for each active field;
- reachable execution nodes used by activation semantics;
- explicit initializer and destruction roots introduced by each field;
- activation-dependency edges; and
- stable counts by direct, initializer, destruction, dispatch, function-value,
  optional, shared, and array cause.

Witnesses, spans, queues, graph edges, SCC data, and counts remain analysis or
planning-report data. They are not semantic identity and do not travel to the
backend.

## Static-lifecycle certificate and products

### Active-set authority

The compact immutable lifecycle proof must record exact active-field authority
issued from verified preliminary MIR. A likely conceptual split is:

```rust
struct StaticActivationAuthority {
    active_fields: Vec<StaticFieldId>,
}

struct StaticLifecycleCertificate {
    activation: StaticActivationAuthority,
    effects: StaticLifecycleAuthority,
}
```

Exact Rust names are implementation details. The invariant is that the
active-field set is a private, sorted, unique, immutable baseline beside the
existing normalized root-effect authority. Optimization passes receive no API
to edit either part.

The effect authority needs roots only for active initializer and destruction
work. Full preliminary analysis can remain in the discarded planning report;
inactive roots do not need long-lived authority because they have no runtime
lifetime to justify.

### Planned and final MIR shape

Preliminary MIR remains definition-complete. Every accepted explicit static
initializer has a body, stable identity, full ownership/lifetime structure,
and ordinary verification regardless of activation.

The planned lifecycle schema changes from one definition per declared field to
one definition per active field. Its activation vector covers that exact set
once. Synthesis moves only active explicit initializer bodies into the final
coordinator and creates destruction regions only for active fields. Inactive
initializer bodies are consumed with the preliminary product but deliberately
not published into final MIR.

Static declarations, initializer identities, classes, generic applications,
types, visibility, source spans, and field IDs remain complete and stable.
Final MIR may therefore contain a declared explicit initializer identity with
no executable body or coordinator region because its field is certified
inactive. This is semantic sparsity established before the optimization
pipeline, not a pass-selected deletion.

The meaning of optimization-off completeness changes narrowly: `none` retains
all executable definitions belonging to the active semantic program, but it
does not restore inactive static lifecycle. Preliminary MIR remains the exact
definition-complete product for producer and diagnostic testing.

## Verification boundary

Verification should preserve the existing split between issuance and final
realization while adding exact activation checks.

### Preliminary and issuance verification

- Validate every declaration and every explicit initializer body, including
  inactive ones.
- Independently recompute the canonical activation closure from verified
  preliminary MIR and require exact equality with activation authority.
- Require lifecycle definitions and the activation plan to cover exactly the
  active set and no inactive field.
- Derive dependencies only from active initializer/destruction roots and
  reject self-dependencies or cycles in that active graph.
- Require every static target reached from active lifecycle work to belong to
  the active set.
- Issue exact normalized baseline effect authority only after these checks.

The checker may share exhaustive instruction, terminator, target, and
lifecycle extraction. It must not trust an unchecked active set or the
planner's already-solved closure as its proof of exactness.

### Final realization verification

- Require coordinator activation, initializers, destruction regions, plan,
  definitions, and active authority to agree exactly.
- Preserve the existing subset relation between final root effects and
  preliminary baseline authority.
- Recompute final executable reachability from entry plus the certified active
  coordinator and reject any reachable static access to a field outside the
  active set.
- Continue validating every physically retained ordinary body even when it is
  unreachable, but allow such a body to mention an inactive static because it
  cannot execute in the verified closure.
- Reject a pass that adds a call, callable address, lifecycle edge, or static
  access capable of making an inactive field executable.

Removing the last final-MIR access to an active field does not deactivate it.
Its initializer and shutdown remain observable according to the frozen
pre-optimization contract. A future proof for eliminating an active but
observationally inert lifecycle is separate work.

## Optimization and reachability interaction

The semantic activation analysis and target-independent final-MIR reachability
share extraction vocabulary but answer different questions:

- semantic activation freezes which static side effects belong to the program;
- final reachability determines which executable definitions are needed to
  realize that already-frozen program.

After active-subset synthesis, final whole-world reachability roots the selected
entry and every active coordinator activation and shutdown obligation. It no
longer sees inactive static work because that work is absent from the final
coordinator. The existing definition-retention pass may then remove ordinary
and member bodies that became unreachable.

`none` skips selectable transformations but does not skip semantic activation.
Pass exclusions and repeated schedules likewise do not affect the active set.
Checkpoint and dump APIs must distinguish preliminary declared initializers,
certified active lifecycle definitions, physically retained final definitions,
and emitted machine artifacts.

## Backend and artifact behavior

The backend continues receiving only verified final MIR. It emits the private
program initializer and finalizer from active coordinator regions, so inactive
explicit initializer code and inactive destruction work never enter those
functions.

Production target metadata and slot planning should eventually consume the
certified active-static query. A first implementation may conservatively build
target-private slots for more declarations when complete-emission diagnostics
or unreachable retained bodies need addressable symbols. That does not restore
a source-visible lifetime: no inactive initializer or destructor may be called,
and mandatory machine-artifact retention must remove unreferenced slots from
ordinary emitted output.

This design changes no public runtime ABI, host entry signature, static field
layout rule, object layout, callable ABI, or source-visible native symbol.
Static symbols remain compiler-private. The existing target-generated symbol
walk remains necessary for entry wrappers, helpers, dispatch tables, literals,
trace metadata, and any conservatively planned inactive slot.

## Diagnostics, reporting, and inspection

Parsing, resolution, type checking, privacy, stored-type, copy, ownership, and
preliminary MIR diagnostics remain independent of activation. Inactive code is
not allowed to be malformed merely because it cannot execute.

`STA001` self-dependency and `STA002` cycle diagnostics apply only to the
active static dependency graph. An entirely inactive cycle has no runtime
lifetime to order and is accepted. If a later source edit makes any member of
that component activation-reachable, the complete transitive component and
its existing source-rich evidence participate in diagnostics.

No permanent warning is proposed for an inactive explicit initializer. Such a
warning would make normal pay-for-use library tables noisy, and this proposal
deliberately supplies no eager annotation to silence it. Debug inspection
instead needs deterministic visibility:

- active and inactive static counts;
- active explicit and zero-default counts;
- inactive explicit initializer count;
- activation edges and conservative target counts;
- canonical first trigger and witness per active field; and
- planned activation and reverse-shutdown order over the active subset.

Passes and analysis helpers do not log. Detailed activation dumps remain
separate from structured report events, following existing compiler reporting
policy.

## Niflheim comparison

The sibling Niflheim compiler was inspected for the same boundary. Its current
semantic reachability walker recognizes canonical static-field reads and
writes, but promotes them to declaring-class reachability. Visiting a reachable
class then walks every static field initializer in that class. That model is a
useful demonstration that static access must be an explicit semantic edge, but
its class granularity would preserve the over-activation this proposal is
intended to remove.

Skald already has stable per-field identities, per-field initializer
identities, an evidenced static dependency graph, exact activation order, and
reverse shutdown. The proposed implementation therefore remains field-grained
and reuses canonical execution/lifecycle extraction rather than copying
Niflheim's class-promotion policy.

## Compatibility and migration

This is a source-compatible but behavior-breaking language change:

- imports and declarations continue resolving exactly as before;
- inactive explicit initializer expressions still produce ordinary compile
  errors;
- inactive-only static self-dependencies and cycles cease producing lifecycle
  errors;
- initializer output, panic, allocation, mutation, and cleanup disappear when
  no activation trigger exists; and
- startup and shutdown order are recalculated over the active subset.

Skald has no promise of separate compilation, dynamic loading, reflection, or
external access to private static symbols, so no unseen runtime client can
activate a field outside the closed program. Function values passed through
interop remain covered by their explicit callable-address formation and exact
candidate rules; a future callback or reflection facility must reopen the root
contract rather than silently bypass it.

The standard library and golden corpus should be audited for declarations
whose only purpose is an initializer side effect. In the absence of eager/module
syntax, each intentional effect must become an ordinary reachable operation or
remain unsupported. The migration should include a representative large-table
fixture, such as decimal parsing imported but not used, to prove both semantic
behavior and material backend reduction.

## Decision register

| ID | Question | Proposed direction | State |
|---|---|---|---|
| [RSL1](#rsl1--separate-declaration-from-runtime-activation) | Does every declared static acquire a lifetime? | No; declaration remains whole-world, activation is entry-reachability gated | **Proposed** |
| [RSL2](#rsl2--define-field-grained-activation-triggers) | What activates a field? | Ordinary static access from canonical activation-reachable execution | **Proposed** |
| [RSL3](#rsl3--freeze-activation-before-selectable-optimization) | Where does activation run? | Mandatory verified preliminary-MIR boundary before lifecycle planning | **Proposed** |
| [RSL4](#rsl4--use-a-coupled-execution-and-field-fixed-point) | How are transitive dependencies found? | One deterministic closure over execution nodes and active fields | **Proposed** |
| [RSL5](#rsl5--freeze-conservative-control-flow-and-target-expansion) | May analysis precision change effects? | No; all structural blocks and current full dynamic target rules are semantic | **Proposed** |
| [RSL6](#rsl6--plan-and-synthesize-only-the-active-subset) | What crosses into final lifecycle MIR? | Definitions, bodies, activation, and shutdown only for active fields | **Proposed** |
| [RSL7](#rsl7--extend-the-lifecycle-certificate-with-exact-active-authority) | How is the set trusted later? | Immutable exact active-field authority issued from preliminary MIR | **Proposed** |
| [RSL8](#rsl8--verify-reachable-static-access-against-active-authority) | What prevents missing activation? | Independent issuance closure plus final reachable-access validation | **Proposed** |
| [RSL9](#rsl9--keep-static-lifetime-independent-of-optimization-policy) | Can passes replan activation? | No; removal preserves activation and newly reachable inactive access is invalid | **Proposed** |
| [RSL10](#rsl10--limit-lifecycle-diagnostics-to-active-fields) | Do inactive cycles fail compilation? | No; ordinary checking remains complete, lifecycle diagnostics use active graph | **Proposed** |
| [RSL11](#rsl11--let-backends-consume-active-lifecycle-with-conservative-slot-fallback) | What may the backend emit? | Active coordinator only; extra private slots allowed temporarily and pruned later | **Proposed** |
| [RSL12](#rsl12--defer-explicit-eager-and-runtime-lazy-features) | How can side-effect-only initialization be requested? | No new mechanism in this design; use ordinary reachable code | **Proposed** |

## RSL1 — Separate declaration from runtime activation

The language should retain complete declaration checking while removing the
implicit rule that import reachability grants every static a lifetime. This is
the core pay-for-use contract and the source-visible behavior change.

## RSL2 — Define field-grained activation triggers

Canonical ordinary access to one `StaticFieldId` activates that field, not its
declaring module, class, base classes, sibling fields, or generic template.
Each closed generic specialization retains its existing distinct static IDs
and activates independently.

## RSL3 — Freeze activation before selectable optimization

Activation runs exactly once after preliminary verification and before
lifecycle planning. It is part of semantic compilation under every profile,
not a selectable MIR pass or backend reachability heuristic.

## RSL4 — Use a coupled execution and field fixed point

Entry-rooted execution exposes static accesses; newly active fields expose
initializer and shutdown execution; that execution can expose more fields.
Solving only callables first or only static dependencies first is incomplete.
The two domains must reach one deterministic least fixed point.

## RSL5 — Freeze conservative control flow and target expansion

All blocks in a reachable preliminary body and the current full-family,
full-conformance, exact-function-type target policies define activation.
Future precision may reduce code but not already-selected static effects.

## RSL6 — Plan and synthesize only the active subset

Lifecycle definitions, order, initializer bodies, and destruction regions in
planned/final MIR cover exactly active fields. Declarations and preliminary
initializer bodies remain complete; inactive bodies do not cross into final
MIR.

## RSL7 — Extend the lifecycle certificate with exact active authority

The compact proof stores a sorted exact active set alongside normalized
root-effect authority. Planning reports retain reasons and witnesses; passes
and backends receive read-only semantic queries, not mutation authority.

## RSL8 — Verify reachable static access against active authority

Issuance independently proves exact activation. Final verification checks the
coordinator against the certificate and rejects every reachable static access
outside it, while permitting unreachable retained diagnostic bodies to mention
inactive declarations.

## RSL9 — Keep static lifetime independent of optimization policy

No final-MIR pass can deactivate a field or suppress its already-selected
effects. A transformation that creates a path to an inactive static fails
verification. `none` and `default` therefore share exact static behavior.

## RSL10 — Limit lifecycle diagnostics to active fields

All ordinary source and preliminary-MIR errors remain complete. Only ordering,
self-dependency, and cycle diagnostics are activation-sensitive because
inactive fields have no runtime lifetime to order.

## RSL11 — Let backends consume active lifecycle with conservative slot fallback

Only active coordinator work may execute. Backend storage and metadata should
become active-query driven, but temporary conservative private slots remain
permitted when they have no initializer/destructor effects and production
artifact retention removes them.

## RSL12 — Defer explicit eager and runtime-lazy features

This proposal adds no syntax or runtime state machine. Intentional global side
effects require a reachable ordinary operation until a separate eager/module
initialization design establishes its own roots, ordering, and diagnostics.

## Verification and test strategy

### Language and activation analysis

- imported but unused explicit, zero-default, final, optional, shared, class,
  array, string, and generic statics remain inactive;
- direct read, write, replacement, immutable borrow, and mutable borrow each
  activate exactly the selected field;
- access through reachable static, instance, virtual, interface, indirect,
  constructor, copy, assignment, and destruction execution activates fields;
- accesses in structurally retained constant-false branches activate fields;
- accesses only in unreachable callables, address formations, or dynamic
  implementations do not activate fields under the frozen target rules;
- active initializer and shutdown effects transitively activate prerequisites;
- unrelated fields in the same class and generic template remain inactive;
- deterministic first reasons, witnesses, order, and counts survive repeated
  and independent-process runs.

### Lifecycle and certificate

- active explicit initializers execute once before entry and active zero-
  default fields begin live without Skald value work;
- active fields shut down in exact reverse order and inactive destructors never
  execute;
- inactive self-dependencies and cycles are accepted while activating the same
  component produces deterministic `STA001`/`STA002` diagnostics;
- malformed missing/extra active authority, lifecycle definitions, activation
  regions, initializer bodies, destruction regions, and order entries fail
  verification;
- every active root effect remains within baseline authority after final-MIR
  transformation;
- a newly reachable inactive static access fails final verification;
- removal of every final access to an already-active field does not alter its
  lifecycle.

### Pipeline, backend, and native behavior

- `none`, `default`, reachability-disabled, and repeated internal schedules
  produce the same active set and native static observations;
- module reads, resolution/type-check statistics, source diagnostics, and
  preliminary MIR stay unchanged by activation;
- final MIR omits inactive static initializer definitions and coordinator
  regions under every profile;
- backend observers never visit inactive initializer/destruction bodies;
- emitted assembly contains no inactive program-initializer calls or
  target-private artifacts reachable only through them;
- imported-but-unused decimal parsing omits `_EiselPowers._words` activation,
  allocation, cleanup, and transitive table artifacts;
- active static panic, output, ownership, allocation, replacement, and
  destruction behavior remains byte-for-byte equivalent to the current
  contract; and
- complete golden, native, runtime-trace, malformed-MIR, determinism, MSRV,
  documentation, and artifact-free repository gates pass.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| An omitted static-access or lifecycle edge suppresses required initialization | Reuse exhaustive shared extraction, variant-focused tests, exact independent issuance verification, and final reachable-access checks |
| Optimization level changes initializer side effects | Run activation before the selectable pipeline and make its certificate immutable |
| A retained unreachable body references missing static storage | Permit conservative private slot planning or ensure production pruning removes the body and slot together; verify no reachable body has such an access |
| Class-level reachability activates unrelated library tables | Use exact `StaticFieldId` triggers; class/type reachability is explicitly not activation |
| Future devirtualization silently changes static behavior | Freeze activation target expansion as language semantics; later analyses optimize code only |
| Inactive lifecycle cycles hide ordinary source errors | Continue resolving, typing, lowering, and structurally verifying every preliminary initializer body |
| Users relied on static initializers for registration | Document the breaking contract; require ordinary reachable bootstrap code until explicit eager/module initialization is separately designed |
| Certificate and coordinator encode drifting active sets | Store one immutable sorted authority and require exact coverage at planned and final boundaries |
| Static lifecycle and final reachability become cyclic owners | Static lifecycle owns activation policy on preliminary MIR; reachability owns shared dependency extraction and final executable closure |

## Alternatives considered

### Keep every imported static eager

This preserves current behavior but keeps unused side effects and makes
pay-for-use standard-library tables impossible. It also turns all static work
into roots that limit future whole-program optimization.

### Make static activation a selectable optimization

Rejected because initializer output, panic, allocation, mutation, cleanup, and
cycle diagnostics would differ between `none` and `default`.

### Initialize lazily on first runtime access

This resembles Java, C#, or Swift type/global initialization but would require
runtime state, first-access guards, recursive-initialization semantics, failure
caching, and eventually synchronization if Skald's execution model changed.
Closed-world single-threaded Skald can select the needed eager subset at
compile time and retain simpler access and shutdown behavior.

### Allow the compiler to activate any conservative superset

Rejected because an extra initializer may have observable side effects. The
activation set must be exact under a frozen conservative source contract, not
an implementation-quality choice.

### Remove only provably pure inactive statics

This preserves current eager semantics but requires a much stronger effect and
alias proof. Allocation failure, ownership, destruction, mutable shared
pointees, panic, and I/O make the useful proof boundary substantially harder.
It remains possible later for fields already active under this proposal.

### Activate all statics of a reachable class

This is Niflheim's current coarse semantic reachability shape. It is sound but
does not solve the motivating case when one broadly used library class owns
several independent feature tables. Skald's field identities make the finer
boundary natural.

### Recompute activation after every optimization

Rejected because pass order and analysis precision would become observable
through startup, shutdown, and diagnostics. Static activation is semantic;
final callable and artifact reachability remain optimizations.

### Delete inactive declarations and compact identities

That would spread a language-semantic change into global identity rewriting,
module inspection, diagnostics, types, metadata, and dumps. Stable declarations
and sparse executable lifecycle provide the benefit without that risk.

## Relative effort and recommended delivery shape

Overall effort is **large**. The fixed-point algorithm is modest; the work is
in moving the complete-field assumption out of planning, proof, synthesis,
verification, backend storage, dumps, tests, and current documentation without
weakening inactive-body checking.

| Delivery slice | Relative effort | Primary result |
|---|---|---|
| Contract characterization and activation analysis | Medium to large | Deterministic field-level active set and explanations without behavior change |
| Active-set certificate and independent issuance verification | Medium | Trusted immutable semantic authority |
| Active-subset planning and diagnostics | Medium to large | Ordering and cycle rules apply only where runtime lifetime exists |
| Coordinator synthesis and final verification | Large | Inactive initializer/destruction work is absent and reachable accesses remain safe |
| Final reachability and backend integration | Medium to large | Active roots, storage, helpers, and artifacts agree across profiles |
| Standard-library, golden, migration, and documentation hardening | Medium | Proven semantic transition and representative pay-for-use reduction |

The implementation should first add analysis and inspection beside current
eager planning, proving the proposed active set without changing execution.
Only after exact issuance verification exists should planning and synthesis
switch to the subset. Backend pruning and broad library reduction come after
the final verifier can reject a missing activation independently.

## Confirmation and promotion

RSL1 through RSL12 should be confirmed together. Activation triggers, phase
placement, certificate authority, lifecycle diagnostics, optimization
independence, and backend consumption form one observable correctness
boundary; freezing only the table-removal outcome would leave initializer
side effects and `none` behavior undefined.

After confirmation, promote the source-visible contract into
`docs/language/STATIC_FIELDS.md`, the status matrix, grammar cross-references,
and error semantics. Promote phase ownership into compiler phases, backend,
driver, reporting, debugging, and testing documentation. Then create a
PR-sized implementation roadmap and a separate discoveries record. The
roadmap should preserve behavior-characterization and independent verification
before changing which static lifecycle work executes.
