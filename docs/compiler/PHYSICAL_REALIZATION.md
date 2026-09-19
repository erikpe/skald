# Native Physical Realization

The private x86-64 SysV physical owner expands an exact verified selected
callable, immutable checked placement and checked frame plan into a typed draft.
The frame plan borrows its placement, which borrows its selected publication.
Realization checks both identities, including replacement products with equal
contents. The draft retains that borrow chain; it cannot outlive its authorities.
Production emission still uses the existing backend.

## Concrete representation

Instructions have closed target opcodes, full GPR or XMM register identities,
explicit widths, immediate bits, concrete signed displacements and callable-local
physical block identities. RIP-relative code/data addresses, local-exec TLS
offsets and direct calls retain typed catalog artifact identities. Indirect calls
use the assigned physical register. There is no virtual value operand, symbolic
frame offset, generic semantic operation or assembly-string instruction.

Each instruction group records its selected site, ordered transfer occurrence,
frame protocol action or original selected edge. Metadata dependencies survive
in groups even when a selected boundary emits no instructions. Actual relocation
and call operands also retain their typed references. This separation gives the
independent checker both declared obligations and concrete instructions to
reconcile; the producer's dependency list is not a closure certificate.

## Expansion authority

All locations and scratch come from checked placement; all object and storage
coordinates come from the frame plan. Memory-to-memory transfers use the
transfer's declared working register. Transfers preserve the supplied order,
including parallel-copy cycle resolution. Selected numeric correction CFG is
already explicit: realization expands finite native cells, without adding a
semantic correction, helper, temporary storage or dependency family. Bounded
recipes reject expansion beyond their declared step counts.

Byte multiplication uses its declared widened integer scratch. Native scalar
memory accesses retain the selected scalar width; wider narrow-scalar carriers
are outside the current native selection contract. Floating constants and sign
changes use declared integer scratch and bit transfers. Integer and floating predicates preserve their
selected signedness and parity rules; division and shifts use their fixed
register contracts. TLS expands to the explicit local-exec address sequence.

The entry block pushes the incoming FP, establishes FP, reserves the fixed
aligned body, performs entry transfers and jumps to the selected entry. Saves
and restores are ordinary declared placement transfers at their promised widths.
Returning terminals perform their supplied transfers before restoring SP/FP and
returning. Nonreturning calls retain their declared defensive trap. Edge copies
use administrative forwarding blocks tied to the original successor occurrence;
they add no semantic branch or ABI transition.

Native realization rejects unsupported frame policies and sizes before expansion.
Supported native frames use direct signed-32-bit coordinates; synthetic shared
frame materialization and link-register policies are not native recipes. See
[frame planning](FRAME_PLANNING.md) and [placement checking](PLACEMENT_CHECKING.md)
for the authorities that precede realization.

## Verification and publication boundary

A physical draft is not executable publication authority. The independent target
checker consumes it into an immutable `VerifiedPhysicalCallable` only after all
of the following checks succeed:

- Encoding rules cover every closed opcode, register bank, width, immediate,
  memory form and local branch destination. GPR identities represent low-byte
  views only, so conflicting high-byte/REX combinations cannot be constructed.
  Typed displacement fields and checked frame access bounds remain mandatory.
- Exact selected, placement and frame identities must match the draft's borrow
  chain. Selected sites and successor occurrences account for every ordered
  group, forwarding block, entry action and returning epilogue, exactly once.
- A separate finite acceptance cursor checks each concrete recipe and transfer
  against immutable selected facts, assigned operands/scratch and frame regions.
  It does not call the realizer, its operand decoder or the formatter as an
  oracle. Changes in valid recipes require corresponding independent acceptance
  rules and adversarial tests; general instruction equivalence is not proved.
- Actual relocation/call references must belong to their group's declared
  dependencies, resolve in the frozen typed catalog and agree with the selected
  site's metadata. Administrative groups cannot acquire new dependencies.
- Reachable CFG state establishes FP and fixed body SP, protects the saved FP and
  incoming return address, checks call alignment and restores the incoming
  state at returns. Joins and loops require identical frame state. Original
  callee-saved register bits propagate through full-width integer/SIMD copies
  and frame storage; partial writes invalidate evidence. Calls discard evidence
  in caller-clobbered registers, including all SysV XMM registers. A finite intersection analysis
  converges before strict preservation replay, with checked work bounds.

The verified product retains the exact authority borrow chain. Its cloneable
receipt carries the exact selected parent, a distinct physical publication
identity and typed reference inventory, without retaining instruction bodies.
Equal-content re-verification creates a different publication. Whole-program
closure must still reconcile executable-pass parents and frozen data inventory
before assembly publication.

Immutable text inspection uses canonical identities, concrete instructions and
group provenance. Physical code, placement assignments/transfers and frame
regions are three independent requests; an unrequested section is not rendered.
Quiet checking does not collect dumps or access source services. An immutable
visitor exposes entry, blocks and instructions.

The target-owned formatter accepts one typed instruction and callbacks for
artifact symbols and scoped block labels. A physical-program builder is its only
callable/program owner. It renders independently verified callables into private
temporary fragments, then releases their bodies while retaining exact physical
and selected receipts plus typed dependencies. Fragment text is never parsed or
used as verification evidence.

Finalization requires every retained callable exactly once, reconciles every
physical receipt with the finalized selected program, and checks typed references
against the frozen catalog. Only then are fragments read in canonical callable
order and combined with checked data definitions into an immutable assembly
product. Trace code refers to the runtime-owned versioned TLS symbol. Missing,
duplicate, stale or foreign bodies reject; read, write
and cleanup failures produce errors and the temporary store removes residual
files on drop. External linker spellings are explicit projected inputs keyed by
typed external identities; absent or surplus entries reject before construction.

The private whole-program pilot consumes this closed product after mandatory
selection, placement, frame and physical checks. Its native execution matrix
covers default/minimal MIR schedules, both trace policies and complete/reachable
artifact policies; scalar C probes exercise both call directions and integer/SIMD
bank pressure. Hardening probes cover unordered floating predicates, upper-half
unsigned conversions, signed minimum/floor division, complete-width shift checks,
destructive live ties and values surviving caller clobbers. Requested checkpoints
are deterministic across processes and observation failure cannot publish an
artifact. Receiver, hidden-result and alias-origin components remain native
phase fixtures because their source forms are deliberately outside admission.
The ordinary backend remains unchanged until the separate adoption workstream.
