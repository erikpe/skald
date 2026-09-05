# Convergent Local Constant Propagation Discoveries

Status: open implementation companion for the planned
[convergent local constant propagation roadmap](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md).

The frozen
[design](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md)
owns the reviewed constant graph, carrier certificate, convergence,
static-failure, independent-consumer, proof-transition, logical selection, and
atomic normalization decisions. The
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
cross-domain candidate status and placement.

This file intentionally starts without findings. During implementation, record
only work discovered outside the active task's frozen scope. Each entry should
state the problem, concrete evidence, likely owner and priority, and a bounded
later direction. Small maintainability improvements that directly support the
current task should be implemented in that task instead.
