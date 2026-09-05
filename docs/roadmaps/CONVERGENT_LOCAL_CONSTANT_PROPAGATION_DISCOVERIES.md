# Convergent Local Constant Propagation Discoveries

Status: open implementation companion for the active
[convergent local constant propagation roadmap](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md).

The frozen
[design](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md)
owns the reviewed constant graph, carrier certificate, convergence,
static-failure, independent-consumer, proof-transition, logical selection, and
atomic normalization decisions. The
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
cross-domain candidate status and placement.

CLR0 and CLR1 completed without an out-of-scope finding. CLR1 added the shared
exhaustive storage-use census and the private checked-carrier certificate;
CLR2 added the immutable dependency graph and convergent solver. CLR3 migrated
primitive folding and the bounded algebraic/CFG fact consumers to that shared
solution. During the remaining implementation, record only work
discovered outside the active task's frozen scope. Each entry should
state the problem, concrete evidence, likely owner and priority, and a bounded
later direction. Small maintainability improvements that directly support the
current task should be implemented in that task instead.

## Sibling checked subexpressions introduce an excluded preservation spill

**Evidence:** On the current lowering of
`((8 / 2) + (7 % 3)) / 2`, the first inner result is reloaded from its
protocol-owned result carrier and then stored in a separate `ScalarSpill`
while the second checked subexpression executes. The addition consumes a load
from that preservation spill. It is not one of the operand/result storage IDs
named by any checked terminator, so the frozen carrier rule correctly leaves
it opaque. The convergent solver therefore proves each inner result but not
the addition or outer division.

**Impact:** The solver is complete for its frozen supported graph, including
arbitrarily deep alternating primitive and checked chains which cross only
protocol-owned carriers, but source expressions with independently checked
siblings can lower through an extra excluded edge. The roadmap's later exact
three-protocol fixture cannot become one constant until that mismatch is
resolved.

**Likely owner:** Carrier provenance and MIR expression-preservation lowering,
before dependent checked-protocol rewriting. Do not weaken the certificate
into generic unique-store propagation. Either give these compiler-generated
continuation spills explicit checked-protocol ownership or revise lowering so
the preserved value flows through an already certified protocol carrier.

**Priority:** High before the checked-protocol consumer milestone; outside the
frozen graph/solver scope.
