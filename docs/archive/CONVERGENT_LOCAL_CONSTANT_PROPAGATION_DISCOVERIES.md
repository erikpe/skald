# Convergent Local Constant Propagation Discoveries

Status: resolved and archived with the completed
[convergent local constant propagation roadmap](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md).

The frozen
[design](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md)
owns the reviewed constant graph, carrier certificate, convergence,
static-failure, independent-consumer, proof-transition, logical selection, and
atomic normalization decisions. The
[optimization candidate catalog](../roadmaps/OPTIMIZATION_CANDIDATE_CATALOG.md) owns
cross-domain candidate status and placement.

CLR0 and CLR1 completed without an out-of-scope finding. CLR1 added the shared
exhaustive storage-use census and the private checked-carrier certificate;
CLR2 added the immutable dependency graph and convergent solver. CLR3 migrated
primitive folding and the bounded algebraic/CFG fact consumers to that shared
solution. CLR4 migrated checked folding to one solved snapshot and resolved the
only prior finding below. CLR5 added the typed single-occurrence
proof-transition schedule region, capability, atomic normalization route,
occurrence and checkpoint contracts, and boundary failure ownership without
adding a production logical pass. CLR6 added the immutable constant-left
logical selection plan, exact edge and optional result rewrites, and their
single atomic composition with mandatory normalization. Exact proof records
remain the authority for protocol sites which also happen to be lifecycle
attachment roots: selection preserves those roots and all blocks, storage,
lifetime operations, and non-selected instructions. CLR7 registered that pass,
placed it once at the default transition boundary, added public selection,
stable measurements and checkpoints, and pinned native success/failure parity.
Its production matrix also exposed and fixed definite scalar and function-value
initialization treating disconnected post-selection blocks as executable
predecessors; those flow-sensitive checks now start only from the callable
entry, while structural verification remains exhaustive. The same matrix
showed that post-proof deletion can leave an inert checked-view declaration
with only its lifetime/end markers; final verification now permits that inert
declaration while still requiring a binding for any material carrier use.
CLR5 through CLR8 produced no out-of-scope finding. CLR8 removed obsolete
rollout suppressions, strengthened future-variant defenses, and confirmed the
existing responsibility split without creating another discovery.

There are currently no open roadmap-specific findings.

## Resolved during CLR4

The sibling checked-expression preservation spill was removed without
broadening constant propagation into general storage. Lowering now reloads a
checked result from its existing protocol-owned result carrier when the value
must survive a checked sibling. Carrier certification accepts all exact,
dominated, in-lifetime loads while retaining one store, exact protocol
ownership, exhaustive access classification, and every original alias and
authorization exclusion. Consequently `((8 / 2) + (7 % 3)) / 2` produces all
three candidates from one solution and folds through one callable transaction.
