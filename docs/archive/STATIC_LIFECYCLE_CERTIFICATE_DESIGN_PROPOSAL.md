# Static-Lifecycle Certificate Redesign Proposal

Status: frozen design proposal. SLC1 through SLC12 were confirmed together on
2026-08-30 and promoted into the
[compiler phase and IR contract](../compiler/PHASES_AND_IR.md#frozen-static-lifecycle-certificate-direction).
The
[implementation roadmap](../roadmaps/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
owns delivery; this document preserves the reviewed decisions.

This proposal redesigns Skald's exact static-lifecycle certificate so
target-independent MIR optimization can remove or reshape static effects
without invalidating the lifecycle proof. It also uses the schema change to
separate analysis data from executable MIR, remove redundant lifecycle
encodings, tighten phase ownership, and simplify verification.

The language contract does not change. Static fields remain eagerly activated
in one deterministic whole-program order and destroyed in the exact reverse
order. Static lifecycle cycles remain compile-time errors. No runtime access
guard, lazy initialization, synchronization, new alias rule, or concurrency
contract is introduced.

Skald programs are permanently compiled whole-world for this design, and the
resulting program is single threaded. Whole-world compilation is essential to
the finite call-target and effect analysis. Single-threaded execution
means the redesign needs no concurrent initialization states, atomic guards,
or memory-ordering proof; it does not otherwise weaken the static dependency
requirements.

## Intended outcome

The redesign should provide:

- one exact, optimization-independent lifecycle plan selected from verified
  preliminary MIR;
- an immutable baseline authority describing the normalized static effects
  originally reachable from each lifecycle root;
- final-MIR validation that accepts removal and graph reshaping but rejects a
  previously unauthorized lifecycle-root effect;
- support for unreachable-block elimination, dead-call elimination, target
  narrowing, devirtualization, inlining, and later whole-program callable
  pruning without optimization-dependent source diagnostics;
- no requirement that final direct effects, call edges, source spans, or
  function-value candidate inventories exactly match their preliminary-MIR
  shapes;
- independently checkable lifecycle safety at the backend boundary;
- one canonical activation order, with shutdown order and indices derived
  rather than stored repeatedly;
- one canonical structured coordinator representation rather than parallel
  flat and structured transition forms;
- pass-owned effect graphs, solver statistics, and diagnostic witnesses rather
  than embedding all analysis internals in final executable MIR; and
- narrow APIs through which optimization passes can rewrite executable MIR
  without editing lifecycle proof internals.

## Current boundary and architectural evidence

The current pipeline is:

```text
typed HIR
  -> preliminary MIR
  -> whole-program static-effect inference
  -> static-lifetime planning
  -> planned-MIR verification
  -> lifecycle coordinator synthesis
  -> synthesized-MIR verification
  -> final MIR pass pipeline
  -> backend
```

This phase order is sound and should remain. The limitation is the product
crossing those boundaries.

`MirStaticLifecycleCertificate` currently embeds a complete
`StaticEffectAnalysis`: the exact-signature function-value inventory, every
effect node, every direct effect, every possible target edge, every transitive
effect with a cloned witness path, and an SCC statistic. Final verification
re-extracts the graph from final MIR and requires exact equality for the node
inventory, direct effects, and target edges before checking summary closure and
the lifetime dependencies.

Consequently, removing an unreachable static read, deleting a dead call,
narrowing a dynamic target set, or removing an address formation invalidates
the certificate. Merely replacing a call with its inlined body also changes
the direct graph shape even though the static effects reachable from the
lifecycle root do not grow.

The representation has accumulated adjacent duplication:

- `mir/model/static_lifecycle.rs` owns analysis vocabulary, call-graph edges,
  diagnostic witnesses, lifetime dependencies, the plan, planned phase
  product, executable coordinator, and certificate in one model module;
- `StaticLifecyclePlan` stores activation and exact-reverse shutdown orders;
- every lifecycle definition and static declaration stores indices that can be
  derived from activation order;
- `MirProgramLifecycle` stores flat activation and shutdown transitions while
  `MirStaticLifecycleCoordinator` stores structured regions representing the
  same transitions;
- synthesis builds flat transitions, reparses them into structured regions,
  and retains both forms for later cross-checking;
- transitive witnesses are cloned into summaries and again into lifetime-edge
  evidence; and
- `recursive_components` is a useful analysis statistic but is carried inside
  the semantic certificate.

These checks helped establish the original feature, but exact mirrored data is
now an editing burden rather than an independent source of truth.

## Constraints and non-goals

The proposal assumes and preserves these constraints:

1. Every Skald program is a closed world. All internal callable definitions,
   class implementations, array types, and function-value address formations
   are available to the compiler.
2. The generated program is single threaded. Static activation and shutdown
   cannot race with ordinary Skald execution.
3. Static activation remains eager and program-wide. Removing an entire static
   declaration or its initializer is not authorized merely because no ordinary
   source access remains; proving that its initialization and destruction are
   unobservable is separate optimization work.
4. The source-level cycle rule and its diagnostics are evaluated before
   optimization, so pass selection cannot change program acceptance.
5. Optimization passes remain responsible for semantic equivalence. This
   certificate proves static-lifecycle safety, not that an optimizer changed a
   call to the semantically correct call.
6. Alias-analysis improvements, language-contract changes, SSA conversion,
   dense-identity rewriting, and the general optimization-pass framework are
   outside this proposal.
7. Target-specific machine effects remain backend concerns. This certificate
   covers target-independent MIR static-field lifetime safety.

## Design principles

1. **Prove the semantic property, not an incidental graph shape.** Lifecycle
   order depends on effects reachable from lifecycle roots, not on whether an
   effect is direct, called, devirtualized, or inlined.
2. **Issue the baseline proof before optimization.** The unoptimized program
   determines acceptance, diagnostics, and the stable lifecycle order.
3. **Permit monotone realization.** Final MIR may realize fewer authorized
   effects, but not a new normalized effect for a lifecycle root.
4. **Keep proof authority immutable.** Passes rewrite executable MIR; the
   pipeline extracts and checks their realized effects centrally.
5. **Use one canonical representation per fact.** Reverse order, indices,
   transitions, dependencies, and summaries should be derived when cheap.
6. **Separate semantic identity from evidence.** Spans and witness paths
   explain a fact but do not define whether two lifecycle effects are equal.
7. **Retain a real backend trust boundary.** A backend consumes only a product
   whose ordinary MIR and lifecycle realization have been checked.
8. **Exploit the actual language boundary.** Closed-world target enumeration
   is part of the design; speculative open-world escape hatches are not.
9. **Keep deterministic behavior.** Canonical IDs and stable ordering govern
   authority sets, derived dependencies, dumps, and diagnostics.
10. **Make invalid states harder to construct.** Phase-specific products and
    private constructors should replace mutable bundles of mutually dependent
    vectors.

## Vocabulary and formal invariant

This proposal uses the following terms:

- **lifecycle root** — an explicit static initializer or an implicit class or
  array lifecycle operation used while activating or destroying a static
  field;
- **direct effect graph** — static accesses and possible call/lifecycle edges
  extracted from one MIR product;
- **normalized effect fact** —
  `(target field, access kind, root phase, lifecycle-owned flag)`, deliberately
  excluding span, witness, directness, edge kind, and intermediate callable;
- **baseline authority** — the exact normalized facts reachable from each
  lifecycle root in verified preliminary MIR;
- **realization** — the normalized facts reachable from each lifecycle root in
  the current final MIR;
- **dependency** — `target -> owning static field`, derived from an authorized
  or realized root fact after applying the existing self/publication rules;
- **plan** — the deterministic activation order; shutdown is its exact reverse;
  and
- **evidence** — a source span and deterministic path used for diagnostics or
  inspection, not for semantic equality.

For a MIR product `P` and lifecycle root `r`, let `effects(P, r)` be the
conservative whole-world set of normalized effect facts reachable from `r`.
Let `B[r]` be the stored baseline authority and `O` the selected activation
order.

At certificate issuance:

```text
effects(preliminary_mir, r) = B[r]             for every lifecycle root r
dependencies(B) are valid under O
all existing self-access and publication rules hold
```

After synthesis and after effect-changing optimization:

```text
effects(final_mir, r) subset-of B[r]           for every lifecycle root r
dependencies(effects(final_mir, *)) are valid under O
all existing self-access and publication rules hold
```

The second order check is intentionally retained even though it follows from a
valid immutable baseline. It gives the backend boundary a direct safety check
against the executable program and catches certificate or phase-product
corruption with a precise error.

Direct effects and call edges are not compared across phases. That narrower
relation would reject valid inlining: the caller gains a direct instruction
that was previously reached through an edge. Root-reachable normalized facts
remain unchanged and are the relevant lifecycle property.

## Decision register

| ID | Question | Confirmed direction | State |
|---|---|---|---|
| [SLC1](#slc1--separate-baseline-authority-from-final-realization) | What does the certificate mean across optimization? | Exact baseline root authority plus a monotone final realization relation | **Confirmed** |
| [SLC2](#slc2--compare-normalized-root-effects-not-direct-graph-shape) | What is compared? | Span-free normalized effects reachable from lifecycle roots, not direct effects or edges | **Confirmed** |
| [SLC3](#slc3--use-distinct-issuance-and-realization-verifiers) | Where is exactness checked? | Exact planned-MIR issuance verification and subset final-MIR realization verification | **Confirmed** |
| [SLC4](#slc4--derive-targets-from-each-whole-world-mir-product) | How are dynamic calls handled? | Rebuild conservative targets from the MIR product under verification | **Confirmed** |
| [SLC5](#slc5--make-proof-data-immutable-to-optimization-passes) | Who may change the proof? | Planner-owned opaque authority; passes receive no proof mutation API | **Confirmed** |
| [SLC6](#slc6--store-one-canonical-lifecycle-schema) | Which lifecycle forms are stored? | Definitions, one activation order, structured coordinator regions, and baseline authority only | **Confirmed** |
| [SLC7](#slc7--separate-analysis-proof-and-executable-module-ownership) | Where do the types live? | Pass-owned analysis/evidence, MIR-owned compact proof and executable schema | **Confirmed** |
| [SLC8](#slc8--keep-witnesses-and-statistics-out-of-semantic-identity) | What happens to witnesses and metrics? | Planning/report sidecars; deterministic evidence retained only where it explains diagnostics | **Confirmed** |
| [SLC9](#slc9--freeze-diagnostics-and-order-before-optimization) | Can optimization change acceptance or order? | No; profiles share preliminary analysis, diagnostics, and plan | **Confirmed** |
| [SLC10](#slc10--centralize-invalidation-and-reverification) | How do future passes interact? | Declare lifecycle-effect preservation or invalidation; the pipeline reanalyzes centrally | **Confirmed** |
| [SLC11](#slc11--strengthen-phase-products-and-backend-entry) | How is the trust boundary encoded? | Sealed verified phase products; backend entry requires a verified final-MIR view | **Confirmed** |
| [SLC12](#slc12--migrate-by-invariant-before-adding-optimizations) | How is this delivered safely? | Establish parity and malformed-MIR coverage before the first effect-changing pass | **Confirmed** |

## SLC1 — Separate baseline authority from final realization

The certificate should stop claiming to be an exact description of every
later MIR graph. It should instead be the immutable authority issued from the
accepted preliminary program.

The authority is keyed only by lifecycle roots. Per-callable transitive
summaries remain useful to the planner's efficient solver, but they are an
implementation technique rather than the final proof property. Final
verification starts from the actual initializer and destruction roots and
checks what those roots can reach in the current MIR.

This distinction preserves a stable, conservative plan while allowing the
executable realization to shrink. It also gives the two products clearer
names: a baseline proof is stored; a current realization is derived.

An optimization that introduces a new root effect is rejected even if the
existing activation order happens to accommodate it. This monotone restriction
is cheap, catches an important class of optimizer mistakes, and keeps the
meaning of the baseline certificate stable. Ordinary optimizer equivalence
remains outside the certificate.

## SLC2 — Compare normalized root effects, not direct graph shape

The comparison key should be a dedicated value similar to:

```rust
struct StaticLifecycleEffectFact {
    target: StaticFieldId,
    access: StaticAccessKind,
    phase: StaticEffectPhase,
    lifecycle_owned: bool,
}
```

The exact enum names may change during implementation, but these four semantic
dimensions must remain distinct. In particular:

- moving an access across initializer publication changes its phase and is not
  silently accepted;
- an ordinary static access cannot become the privileged unpublished
  initializer destination;
- read, write, replace, initialize, and destroy distinctions remain available
  to the lifecycle rules and future analyses; and
- source spans and witness routes may change through inlining without creating
  a new semantic fact.

The root is the authority-map key rather than part of every fact. Directness,
intermediate nodes, call-edge kinds, and target-set inventories are deliberately
absent. They are graph-analysis inputs, not lifecycle order requirements.

This permits inlining, outlining reached only from an already authorized root,
call-edge replacement, and dynamic-target narrowing as long as the root's
normalized effect set does not grow.

## SLC3 — Use distinct issuance and realization verifiers

Two explicit verifier entry points should replace one verifier with ambiguous
exactness:

1. `verify_planned_static_lifecycle` independently extracts preliminary MIR,
   computes root-reachable normalized facts, requires exact equality with the
   stored baseline authority, derives all required dependencies, and checks
   the activation order and self/publication rules.
2. `verify_static_lifecycle_realization` independently extracts final MIR,
   computes root-reachable normalized facts, requires a subset of the immutable
   authority, and directly checks the realized dependencies against the frozen
   order.

The planner may retain its SCC-based least-fixed-point solver and deterministic
witness selection. The verifier should use a simpler checker-oriented graph
walk or separately structured closure implementation. Sharing instruction and
terminator extraction is desirable; sharing an unchecked solved summary is
not.

The final checker may initially trade some performance for clarity by walking
from each lifecycle root. If profiling later shows this matters, SCC
condensation or memoized root closures can be added behind the same contract.
Final verification must always run once before the backend even when pass
metadata says every pass preserved effects.

## SLC4 — Derive targets from each whole-world MIR product

Indirect, virtual, interface, constructor, copy, finalization, cleanup, and
array-lifecycle edges must be conservatively reconstructed from the MIR product
being checked.

For function values, the exact-signature address-taken candidate index becomes
an extraction input or analysis sidecar rather than certificate identity. The
preliminary analysis sees all original formations; final analysis sees all
surviving formations. Removing an address formation may therefore narrow the
realized target set without editing the baseline authority.

This is sound because the program is closed-world and Skald function values
cannot acquire an unknown external Skald target. If a future interop feature
invalidates that premise, it must introduce an explicit conservative external
effect summary or reopen this decision; silently treating an unknown target as
effect-free is forbidden.

Callable retention should have its own owner. Today the backend emits all
executable definitions, so removing candidate inventory from the lifecycle
certificate does not change emitted bodies. A future reachability pass must
root surviving address-taken callables explicitly and verify that policy as a
reachability property, not overload lifecycle proof data with a second meaning.

## SLC5 — Make proof data immutable to optimization passes

Only lifecycle planning may construct baseline authority. Certificate fields
and constructors remain private, and no optimization-pass context exposes a
mutable certificate reference.

Passes operate on executable definitions, initializer CFGs, and coordinator
regions through the eventual MIR rewrite API. A pass reports whether it can
change static effects or call reachability; it never patches authority,
dependencies, target inventories, or witness paths to make verification pass.

The pipeline, not the transformation, extracts the post-pass realization and
checks the relation. This keeps the safety policy in one place and makes pass
selection independent of proof bookkeeping.

## SLC6 — Store one canonical lifecycle schema

The redesign should remove adjacent redundant lifecycle forms while the schema
and verifiers are already changing.

The canonical planned data should be:

- one definition per static field, keyed by stable field identity;
- one activation-order vector covering those definitions exactly once; and
- one immutable baseline authority map keyed by lifecycle root.

Shutdown order is always `activation_order.rev()` and should be exposed as a
derived iterator or view. Activation and shutdown indices are derived position
maps, not fields copied into declarations and definitions. Required dependency
pairs are derived from root authority and definitions rather than stored beside
the facts that imply them.

The canonical executable data should be the structured activation and
destruction regions consumed by verification and the backend. Flat begin,
publish, finish, and destruction transition vectors should not coexist as a
second stored encoding. Dumps may flatten structured regions for readability,
but that rendering is derived.

Removing redundant storage does not remove checks. Verifiers still require
complete unique field coverage, correct initialization modes and types,
activation-region order, publication dominance, exact reverse destruction,
legal cleanup, and destination non-escape. The difference is that they check
one representation against semantic requirements instead of checking several
compiler-generated mirrors against one another.

Construction should build field and position maps once. Repeated linear scans
through static declarations during schema construction and verification should
be replaced with explicit indexed views.

## SLC7 — Separate analysis, proof, and executable module ownership

The current monolithic static-lifecycle MIR model should be divided along
ownership boundaries while retaining small facades.

A likely organization is:

```text
passes/static_lifecycle/
  analysis/       graph extraction, target expansion, solving, facts
  plan/           dependency construction, cycle diagnostics, order selection
  verify/         exact issuance and final realization checks
  synthesize/     planned product to structured coordinator
  dump.rs         analysis and planning inspection

mir/model/static_lifecycle/
  mod.rs          facade and stable public re-exports
  proof.rs        normalized authority and lifecycle-root identities
  plan.rs         definitions and canonical activation order
  coordinator.rs  executable activation and destruction regions
  product.rs      sealed planned/final phase products
```

Exact filenames are implementation details, but analysis graph nodes, edges,
candidate inventories, SCC counts, and witness machinery should no longer be
presented as general executable MIR schema. MIR should own only the compact
proof and executable data that cross a phase boundary.

This also removes the current circular-looking ownership in which the pass
facade re-exports analysis types from MIR even though only the static-lifecycle
pass understands them.

## SLC8 — Keep witnesses and statistics out of semantic identity

Cycle diagnostics still require deterministic, source-rich paths. The planner
should continue to select minimum-call-edge witnesses with canonical tie
breaking before optimization. Those witnesses belong to the rejected-program
diagnostic or a planning report sidecar.

For an accepted program, the compact certificate need not clone every edge
path into every transitive summary and then clone it again into dependency
evidence. Exact authority issuance is checked by independent graph traversal;
the authority fact itself is sufficient for later subset comparison.

Detailed analysis dumps can consume a `StaticLifecyclePlanningReport` holding
the direct graph, solved summaries, selected dependency evidence, and metrics.
That sidecar is not executable MIR and is not accepted by the backend.
`recursive_components` similarly becomes a report metric owned by the solver.

If long-lived evidence is later wanted for debugging serialized MIR, use stable
evidence IDs into a deduplicated table. Full cloned witness vectors should not
return to semantic equality.

## SLC9 — Freeze diagnostics and order before optimization

Lifecycle cycle detection, self-access rejection, diagnostic paths, and
topological tie breaking run once on verified preliminary MIR. The resulting
diagnostics and activation order are shared by every optimization profile.

An optimizer may remove the code that motivated a conservative dependency, but
the selected order is not tightened afterward. Replanning would make dumps,
startup order, destruction order, and potentially program acceptance depend on
selected passes. The small possible gain from a shorter partial order is not
worth that instability.

This is especially appropriate for whole-world Skald: the baseline already
contains the complete program rather than a partial module view. Single-
threaded execution does not justify weakening the order because deterministic
destruction and pre-publication access rules still apply.

## SLC10 — Centralize invalidation and reverification

The later pass framework should classify lifecycle interaction at least as:

- `PreservesStaticLifecycleEffects` — cannot change static accesses, lifecycle
  operations, control-flow reachability, or possible callees; and
- `MayChangeStaticLifecycleEffects` — may change any of those inputs.

The second class invalidates a cached realization and triggers central
re-extraction before another consumer requests it. Debug/test pipelines should
be able to verify after every such pass; production may batch adjacent passes
but must verify once at the backend boundary.

This proposal defines the invalidation hook, not the general pass registry,
optimization levels, or analysis manager. The certificate checker must also be
callable directly so the future framework does not become a prerequisite for
soundness.

## SLC11 — Strengthen phase products and backend entry

Relaxing final equality means historical baseline exactness cannot be
reconstructed from optimized MIR alone. The compiler must encode that the
authority was issued and checked before transformations rather than relying on
call-site convention.

Planning should construct an internal draft and return a sealed verified
planned product only after exact issuance verification. Synthesis consumes
that product and preserves its opaque authority. The final MIR pipeline
returns a verified view or phase product only after ordinary MIR and lifecycle
realization verification. Backend entry accepts that verified product rather
than an arbitrary mutable `MirProgram`.

This removes the current driver/synthesizer pattern of verifying the same
planned object at loosely related call sites. Public inspection APIs may expose
read-only definitions, activation order, and authority; construction remains
crate-owned. Test-only malformed-product builders should live with verifier
tests instead of adding general mutation accessors to production types.

No cryptographic seal is part of the design. The trust mechanism is Rust
ownership, private construction, opaque phase products, and mandatory
verification at the phase boundary.

## SLC12 — Migrate by invariant before adding optimizations

The redesign should land with behavior parity before the first real
effect-changing optimization. A safe delivery sequence is:

1. characterize the current accepted/rejected lifecycle behavior, dumps,
   public inspection API, and malformed-certificate cases;
2. introduce normalized facts and root-reachable checking alongside the
   existing exact verifier;
3. issue and exactly verify compact baseline authority from preliminary MIR;
4. switch synthesized verification to the subset realization relation while
   retaining all ordinary MIR and order checks;
5. canonicalize plan/coordinator storage and remove mirrored fields only after
   derived views and verifier coverage exist;
6. move analysis-only types and evidence out of executable MIR ownership;
7. require the verified final product at backend entry; and
8. add a test-only transformation that removes, narrows, and inlines effects to
   demonstrate the contract before scheduling an optimizer pass.

These delivery slices are refined into ordered PR-sized tasks in the
[implementation roadmap](../roadmaps/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md).

## Frozen product model

The final names should follow repository conventions, but the ownership should
approximately be:

```rust
struct StaticLifecycleAuthority {
    roots: Vec<StaticLifecycleRootAuthority>,
}

struct StaticLifecycleRootAuthority {
    root: StaticEffectNode,
    effects: Vec<StaticLifecycleEffectFact>,
}

struct MirStaticLifecyclePlan {
    definitions: StaticLifecycleDefinitionTable,
    activation_order: Vec<StaticFieldId>,
    authority: StaticLifecycleAuthority,
}

struct MirStaticLifecycleCoordinator {
    plan: MirStaticLifecyclePlan,
    initializers: StaticInitializerTable,
    activation_regions: Vec<MirStaticActivationRegion>,
    destruction_regions: Vec<MirStaticDestructionRegion>,
}
```

Collections are sorted and unique at construction. Derived APIs provide
shutdown iteration, activation/shutdown positions, dependency pairs, and flat
dump views without storing them.

`StaticEffectAnalysis`, candidate indices, graph edges, SCC results,
`StaticAccessEvidence`, and diagnostic witness paths remain pass-owned results.
They may be exposed through an explicit planning-report API where useful, but
they do not travel to the backend as if they were executable MIR.

## Verification matrix

The implementation roadmap requires at least these cases:

| Case | Planned issuance | Final realization |
|---|---|---|
| Exact unoptimized program | Accept | Accept |
| Missing baseline authority fact | Reject | Not constructible through the normal pipeline |
| Extra baseline authority fact | Reject | Not constructible through the normal pipeline |
| Unreachable static access removed | N/A | Accept |
| Dead effectful call removed | N/A | Accept |
| Virtual/interface/indirect target set narrowed | N/A | Accept |
| Callee inlined into lifecycle root | N/A | Accept despite changed direct graph shape |
| Source span or witness route changes only | N/A | Accept |
| Access crosses initializer publication | N/A | Reject as an unauthorized phase fact |
| New target field becomes reachable | N/A | Reject |
| Ordinary access becomes lifecycle-owned destination access | N/A | Reject |
| Surviving address formation introduces an indirect target | Included in exact authority | Included in realized analysis; unauthorized effects reject |
| Activation order violates a derived dependency | Reject | Reject |
| Shutdown is not exact reverse activation | Reject | Reject structurally |
| Foreign or duplicate field/root identity | Reject | Reject |
| Optimized and unoptimized profiles | Same diagnostics and activation order | Each realization valid independently |

Mutation tests should target the compact authority, root coverage, phase facts,
coordinator regions, publication dominance, and verified-product boundary.
Golden tests should establish deterministic analysis reports and derived
lifecycle dumps. Optimized/unoptimized native tests should verify equal program
results and lifecycle side effects once transformations exist.

## Alternatives considered

### Run all effect-changing optimization before lifecycle planning

This avoids a cross-phase relation but makes cycle diagnostics, acceptance,
activation order, and destruction order depend on optimization selection. It
also prevents lifecycle facts from guiding later whole-program transformations.
The proposal keeps planning before optimization.

### Require final direct effects and possible targets to be subsets

This works for simple deletion and target narrowing but rejects inlining and
other graph reshaping. Directness and intermediate edges are not the semantic
lifecycle property, so the proposal compares root-reachable normalized facts.

### Rewrite the certificate to exactly match MIR after every pass

Central recertification would permit transformations, but it erases the
optimization-independent authority and could authorize newly introduced
effects whenever the existing order happened to fit. It also makes every pass
boundary produce a new semantic proof product. The proposal keeps the baseline
immutable and derives realization instead.

### Preserve the complete preliminary MIR inside final MIR

This would let final verification reconstruct historical exactness, but it
duplicates a large phase product solely as proof evidence and complicates
identity rewriting and serialization. Opaque verified phase products provide
the required compiler trust boundary without carrying an executable snapshot.

### Keep all transitive summaries and witnesses in the certificate

They make closure locally checkable but duplicate analysis state, tie equality
to lowering shape, and are expensive to rewrite. Independent root-reachable
checking validates the smaller semantic authority directly.

### Add lazy initialization or runtime access guards

This would change the language/runtime model and add state checks to ordinary
access. Whole-world, single-threaded Skald already has enough information to
prove eager deterministic lifecycle safety statically.

## Expected optimization impact

The redesign does not itself speed up generated programs. Its impact is that
the lifecycle proof stops blocking the transformations with the best early
whole-world value:

- unreachable block and dead-call removal can delete effect-bearing regions;
- devirtualization and function-value analysis can narrow conservative target
  sets;
- inlining can replace call paths with direct instructions;
- later reachability can remove unreachable executable definitions while
  retaining address-taken and lifecycle roots under its own policy; and
- CFG cleanup can reshape initializer and destruction call paths without
  rewriting proof witnesses.

It also reduces the amount of metadata that later dense-ID rewriting and CFG
editing must update.

## Effort, risk, and recommended start

Overall effort remains **large** because this is a soundness-boundary and phase-
product change, not because the normalized fact type is complex.

| Work area | Relative effort | Primary risk | Payoff |
|---|---|---|---|
| Normalized root authority and exact issuance | Medium | Omitting a phase or implicit lifecycle route | Establishes the semantic proof |
| Final root-reachable subset verifier | Large | Unsound target expansion or root coverage | Removes the optimization fence |
| Canonical plan/coordinator schema | Large | Regressing publication, cleanup, or backend order | Removes substantial maintenance debt |
| Analysis/evidence ownership split | Medium | Public dump/API churn | Makes later pass code smaller and clearer |
| Sealed phase products/backend boundary | Medium | Driver and test migration | Preserves trust after historical equality is relaxed |
| Parity, mutation, and transformation tests | Large | Missing a malformed or implicit-operation case | Makes the redesign safe to build on |

Start with the normalized fact semantics and dual verification relation. Do
not begin by moving modules or deleting redundant fields: those mechanical
changes are safest once the new invariant is executable in tests. Conversely,
do not defer the schema cleanup until after optimizations depend on the old
certificate API; this redesign is the least disruptive time to remove the
mirrored lifecycle forms.

## Confirmation and promotion

SLC1 through SLC12 were confirmed together on 2026-08-30, including:

- root-reachable normalized effects rather than direct-edge subset;
- immutable baseline authority and optimization-independent planning;
- derived dependencies, reverse order, indices, and flat transitions;
- removal of function-value retention and diagnostic witnesses from
  certificate identity; and
- the sealed phase-product requirement at backend entry.

The durable direction is promoted into
[`PHASES_AND_IR.md`](../compiler/PHASES_AND_IR.md#frozen-static-lifecycle-certificate-direction).
The
[implementation roadmap](../roadmaps/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
owns migration and delivery. This proposal remains the frozen decision record.
