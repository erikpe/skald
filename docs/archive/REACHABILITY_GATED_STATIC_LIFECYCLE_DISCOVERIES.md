# Resolved Reachability-Gated Static Lifecycle Discoveries

Status: resolved and archived on 2026-09-01.

This record preserves the maintainability finding discovered while building
semantic static activation. The living compiler phase and testing documents
remain authoritative for current ownership and behavior.

## Function-value closure mechanics had two solver owners

**Resolution.** A private `passes::reachability::function_values` component now
owns exact-function-type candidate/site coupling. It accepts idempotent reached
execution-node events, handles candidate-first and site-first discovery, keeps
canonical callable-address evidence, and returns newly selected indirect
execution edges in canonical order. Final reachability and semantic static
activation both consume this component; their root policies, normal dependency
closure, static-field activation, witnesses, errors, and result models remain
separate.

Focused owner tests cover both discovery orders, repeated events, exact-type
isolation, canonical evidence replacement, and deterministic target order.
Existing final-reachability and static-activation tests continue to cover the
same reachable function-value target and static-effect behavior through both
consumers.

**Original evidence.** Target-independent final reachability and semantic
static activation intentionally solved different roots and facts, but each
maintained an equivalent state machine in its own `solve` module: reached
callable-address formations added candidates, reached indirect sites consumed
only exact-function-type candidates, and either discovery order had to complete
the same fixed point. Both implementations also owned canonical formation
replacement and late candidate/site coupling, creating a correctness drift risk
before any third whole-program consumer or function-value semantic change.

Explicit eager or module initialization, runtime lazy statics, activation
narrowing from more precise target analysis, declaration or static-slot
identity compaction, and broader effect or alias proofs remain outside this
resolved implementation and require their own concrete proposals before work.
