# Resolver Publication Ownership Roadmap

Status: planned; P01 is next after retrospective R03 verifies current rejection
contracts. The implementation sequence is separate from the retrospective.

The [publication decision](CLEANUP_RETROSPECTIVE_REVIEW.md#r02--publication-acceptance-decision)
requires owned product selection and exhaustive final assembly to complete
A09's outstanding publication goal. It supplies the field inventory,
alternatives, rejection policy, and consumer preconditions for this roadmap.

## Scope and invariants

- Replace manual mutation of a complete candidate with private owned selection
  products. Keep public `ResolvedProgram` and `ResolveOutput` interfaces stable.
- Retain ordinary snapshot capture points, identity allocation, source ordering,
  diagnostic ordering, all-class-family rejection, interface validation after
  class selection, and independent class survival after interface rejection.
- Preserve partial error evidence: error-bearing output is not a closed program
  for type checking. Do not rebuild ordinary resolution, compact identities,
  clear retained metadata, or add per-specialization salvage.
- Preserve successful dumps and executable behavior. No performance claim is
  required; avoiding extra restore clones is secondary to explicit ownership.
- Use facade-oriented private Rust modules. Do not add a universal transaction
  abstraction, public phase seals, or a new specialization engine.

## Progress

- [ ] P01 — Partition candidate ownership without changing selection
- [ ] P02 — Select products and assemble the result once
- [ ] P03 — Verify extension obligations and close the migration

## PR-sized implementation sequence

### P01 — Partition candidate ownership without changing selection

**Purpose:** make every retained or replaceable field have an explicit owner.

- [ ] Map all current fields to retained request/declaration evidence, class
  declarations/hierarchy, interfaces, and completed bodies/dispatch. Keep
  specialization transition metadata explicitly owned beside the selectors.
- [ ] Introduce private products with exhaustive conversion at the current
  assembly boundary, preserving ordinary snapshot timing and all field values.
- [ ] Define the borrowed validation view needed to read candidate products
  and later class-selected products without constructing another independently
  mutable complete program. Keep capability evaluation over resolved facts;
  any required adapter must not introduce a reverse phase dependency.
- [ ] Document the private ownership and diagnostic-output contract.

**Tests:** existing publication/requirement tests plus R03's regressions; check
successful dumps and module permutations. Run `make check` and `make msrv-check`.

**Exit criteria:** every field has an explicit owner and conversion, with no
wildcard destructuring or default-filled catch-all; behavior is unchanged.
The validation-view API is concrete before changing its consumers in P02.

### P02 — Select products and assemble the result once

**Purpose:** replace the hand-maintained rollback mutation list.

- [ ] Validate the candidate class family, select candidate or saved ordinary
  classes/hierarchy and the matching bodies/dispatch bundle, then validate
  interfaces against that selected view.
- [ ] Preserve failed identity transitions and diagnostics without compacting
  tables. Select ordinary or candidate interfaces independently as specified.
- [ ] Move chosen owned products into one exhaustive final assembly. Remove
  obsolete rejection mutators and avoid cloning saved products merely to move
  them into rejection output. Retain necessary pre-mutation snapshots.
- [ ] Update living phase/publication documentation with the implemented model.

**Tests:** the complete retrospective rejection matrix, exact diagnostics,
ordinary preservation, dependent failure, independent survival, and populated
virtual-family clearing. Run `make check` and `make msrv-check`.

**Exit criteria:** selection owns replacement coherently; no complete mutable
candidate is incrementally rolled back. Error evidence and success products
match the characterized policy.

### P03 — Verify extension obligations and close the migration

**Purpose:** establish the new endpoint and remove the architectural blocker.

- [ ] Review all fields against the decision inventory; verify adding a field
  requires an explicit disposition in owned products and final assembly.
- [ ] Review diagnostic consumers and driver error gating; ensure no new closed-
  program assumption was introduced for rejected output.
- [ ] Reconcile A09's detailed outcome and inventory, resolve the publication
  discovery, and record remaining independent work without expanding scope.
- [ ] Archive this roadmap, repair links and indexes, and leave only current
  contracts in living documentation.

**Tests:** `make check` from a clean checkout or artifact-free snapshot,
`make msrv-check`, documentation validation, and diff hygiene.

**Exit criteria:** publication ownership is implemented and tested, the original
manual field-maintenance hazard is addressed, and dependent work has a clear
accepted boundary. No measured speedup or fully closed error IR is claimed.

## Ordering and dependencies

P01 follows retrospective R03's characterization and R02's design decision;
P02 consumes P01's ownership/view API; P03 accepts the final implementation.
The retrospective may close with this roadmap still planned. Changes adding
candidate-dependent tables or altering rejection must satisfy this prerequisite;
independent compiler work need not wait. Additional design discoveries belong
in the indexed retrospective discoveries record, not hidden extra tasks here.
