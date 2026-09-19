# Checked Frame Planning

The private `backend/frame` owner plans storage from an exact immutable
`CheckedPlacement`. A plan borrows that placement and rejects a different checked
placement, even when both borrow the same selected callable. Frame publication
does not certify physical instructions; realization and independent physical
verification remain separate consumers.

## Storage and ABI authority

Semantic and trace objects retain their selected identities. Homes, spills,
callee saves and parallel-copy scratch retain placement storage identities,
purposes, representation widths and declared lifetimes. Objects precede storage
in deterministic identity order. Every local requirement receives a separate
region, including transfer-local scratch; there is no lifetime-based reuse.

Logical signature-local ABI slot shapes specify component representations.
Explicit target area layouts independently specify slot offsets, footprints,
alignment, total extent and padding. Shared validation checks shape agreement,
power-of-two alignment, sufficient footprints, nonoverlap and checked bounds.
An ABI-area object must have exactly the area's extent and alignment. Native
verification independently reclassifies signatures and checks canonical layouts.
Representation byte width cannot determine an ABI slot's stride.

ABI objects overlay their protocol areas rather than allocate private locals.
Incoming slots are independent of frame size. Calls and explicit ABI locations
determine the maximum outgoing area; unused catalog signatures do not enlarge
the frame. Outgoing signatures share that fixed area. A target with memory result
slots receives a separate shared maximum result area. Callee-save storage uses
the target's promised view width, including partial SIMD preservation, rather
than the largest overlapping resource view. Both baseline and manual placements
use the same planner.

## Native stack policy and bounds

The SysV entry SP has remainder eight modulo sixteen. Pushing the old frame
pointer establishes an aligned FP; the return address is at FP+8. Incoming
argument slots begin at FP+16, with eight-byte stride even for byte and boolean
components. Incoming extent is the exact slot count times eight. Outgoing
extent rounds that count times eight to sixteen bytes.

Locals use negative FP offsets. The body size rounds locals plus the maximum
outgoing/result areas to sixteen bytes. Outgoing slots use body-SP offsets
starting at zero; SP stays fixed throughout the body. The frame uses neither the
red zone nor dynamic stack realignment. Alignment above sixteen is rejected.
Checked arithmetic rejects overflow, frames above signed-32-bit size, and memory
access extents outside signed-32-bit displacement range. Alignment rounding can
reject a size that would fit before rounding. Incoming offsets also obey the
displacement bound.

## Address recipes

Planning checks actual placement transfers, memory operands and selected object
references before returning a plan. Direct recipes require the complete access
extent to fit the target displacement range. Native supported frames use direct
recipes; an unsupported extent produces a structured frame error.

A target may opt into a bounded object-address materialization recipe. Its
policy identifies the dedicated declared address-scratch group, supported range
and total address-plus-access step count. The selected bounded bundle must
already declare pointer-width integer scratch in that group, and checked
placement must assign it. The plan records the exact assigned view and recipe
bound. Multiple object materializations count against the bundle's total bound.
The target must reserve that group for this recipe; planning cannot repurpose
arbitrary bundle temporaries. Transfers and memory operands currently require
direct addressing and cannot borrow a nearby bundle's scratch. A new target
requiring far transfers needs a reviewed explicit transfer recipe before adoption.

Synthetic tests use narrower displacement bounds, a sixteen-byte header with a
saved link-register role, separate memory-result slots, and partial-width saves.
They exercise successful materialization and bounded rejection without claiming
an executable AArch64 implementation.

## Physical-state checkpoint

Private native [realization](PHYSICAL_REALIZATION.md) consumes the exact
selected, checked-placement and frame products. It may emit only declared transfers, selected recipes and frame
protocol actions. It cannot add homes, saves, address scratch, calls, traps or
semantic control-flow edges. Object addresses and location offsets come from frame
queries; generated prologue and epilogue actions retain typed frame provenance.

Independent native [physical verification](PHYSICAL_REALIZATION.md) establishes
the following rules for the supported SysV policy. Other target policies must
provide their corresponding checker:

- Entry establishes the declared header, FP and fixed body SP before local
  accesses. Native caller SP alignment and FP+8 return-address provenance remain
  distinguishable from local storage.
- Every reachable CFG join has identical SP/FP and header state. Returning calls
  preserve that state and use the aligned fixed outgoing area. Nonreturning calls
  have no invented epilogue or successor.
- Original promised resource bits are saved at their promised widths before
  destructive use. Returning paths restore them before dismantling the frame.
  Result resources are not accidentally destroyed by restores or address scratch.
- Epilogues restore SP from FP, restore the incoming FP and return using the
  declared return-address role. Native return leaves caller SP advanced past its
  return address. Link-register targets instead restore their saved link role.
- Each concrete access matches its declared region and recipe. Address
  materialization uses only its recorded scratch and bounded steps.
- Administrative forwarding blocks preserve frame state and retain the original
  selected edge/transfer provenance; they cannot invent an ABI transition.

The independent native checker enforces these acceptance rules. The frame
planner establishes layout and addressing requirements, not physical execution
or CFG-state correctness.
