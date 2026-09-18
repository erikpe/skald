# Placement representation and checking contract

Placement is private backend data over a verified selected callable. The shared
`backend/placement` owner currently implements drafts and structural validation.
Availability checking, checked-placement publication, producers, frames and
physical realization remain planned. Drafts never authorize code generation.

The independent checker must implement the following contract. The test-only
specification oracle exercises its difficult state transitions without publishing
authority or relying on a producer's availability map.

## Coordinates and storage

`PlacementDraft` borrows the exact `VerifiedSelectedCallable`. A consuming selected
edit cannot coexist with a usable draft. Snapshot comparison uses the selected
receipt's private publication identity, context and callable; equal graph IDs,
equal instructions or equal callable keys cannot certify a replacement.

Assignments use stable selected coordinates:

| Coordinate | Meaning |
| --- | --- |
| Input index | Location of the incoming ABI value before entry transfers |
| Block and parameter index | Destination location after simultaneous parameter rebinding |
| Instruction/terminal and operand index | Location at that specific descriptor operand event |
| Block, outgoing edge occurrence and argument index | Source location at edge entry, before edge transfers |
| Instruction/terminal, scratch group and index | Resource reserved for the entire declared bundle |

One operand index has exactly one event, reconstructed from its role and timing.
There is no permanent value-to-register mapping. Scratch groups are expanded by
their declared counts. Every assignment is required, including those in
unreachable blocks; unknown coordinates and duplicates reject.

Transfers occur at entry, before/after whole nonterminal instructions, before
terminals, or on one outgoing edge occurrence. There is no after-terminal point
or transfer inside an atomic/bounded bundle. Two edges to the same successor are
different occurrences. Transfer vectors are ordered executable sequences; they
are not an implicit parallel-copy resolver. Selected CFG normalization precedes
placement and requires fresh selected publication.

Locations are resource views, signature-keyed ABI slots, or placement-local
symbolic storage IDs. Semantic/trace objects have their own selected object IDs
and cannot serve as value homes. Placement storage records representation,
extent, alignment, purpose (home, spill, transfer scratch or callee save), and
whole-callable or single-transfer-point lifetime. There are no frame offsets or
slot reuse claims. Structural validation checks IDs, coordinates, representations,
storage extent/alignment and transfer types. It does **not** check opcode location
constraints, reservations, lifetimes, aliasing, content availability, ABI roles,
scratch recipes or equivalence. Its `Result<(), PlacementError>` is not a seal.

A transfer names either a selected value or the distinct original incoming
contents of a preserved resource, source/destination representations and
locations, kind and explicit working views. Save/restore transfers cannot invent
a selected value ID for those incoming bits. `Copy` keeps the representation;
`Bitwise` may carry equal-width bits between integer and floating storage/banks.
Neither changes numeric value or widens/truncates it. Address/signature identity
cannot be manufactured by bit reinterpretation. Only selected instructions may
perform semantic conversions. A target move recipe must declare all scratch and
prove that the sequence preserves the named bits, including memory-to-memory
moves. Writing incoming ABI slots is forbidden. ABI slots are temporary protocol
locations, rather than general persistent homes.

## Finite abstract state

For an exact selected snapshot, construct a finite location universe from all
resource views, declared storage and ABI slots used by its facts/draft. Resource
width/bank is in the view; storage/ABI representations are explicit. Each location
contains a set of identities whose complete bits are **definitely** present:

- `Value(v)` denotes the current dynamic definition of a selected value.
- `Preserved(view)` denotes original incoming bits the target requires restored
  on return, at the exact promised width. These are separate from selected values.

Multiple identities at a location express proven equal bits, for example two
parameters receiving the same argument. An empty set means unknown, not zero.
The top set is all identities, used only to initialize reachable nonentry blocks
for fixed-point iteration; it is never evidence before convergence. Location/type
legality is checked separately. Semantic memory contents are not in this lattice.

Writes conservatively clear every overlapping location before establishing the
new complete contents. Resource overlap comes from units, not bank membership or
register spelling. A narrow write cannot preserve or establish a wider token;
32-bit zero extension likewise does not restore a previous 64-bit identity.
A unit clobber clears every view touching it. A synthetic target can represent a
partly preserved wide resource with separate low/high units: killing only high
units preserves a low view and invalidates a wide view. The current x86 catalog
uses conservative whole-register units, including byte writes.

Symbolic storage IDs have distinct extents until a future checked reuse scheme.
ABI signatures describe slot **types**, not disjoint physical storage: slots in
the same area at the same index alias across signature shapes. Placement checking
reconstructs relative slot footprints from target ABI facts, including width and
stride; frame checking later verifies that concrete offsets obey those footprints.
Call late-clobber processing invalidates all outgoing/results ABI contents and
the declared caller-clobbered resource units. Incoming stack slots are separate.
An outgoing slot cannot retain a value across a call by changing its signature.

Definitions implement epochs without unbounded iteration counters: immediately
before establishing `Value(v)`, remove that identity from **every** location.
Then kill the result location's aliases and establish its new bits there.
Consequently a home saved on an earlier loop iteration cannot masquerade as the
new definition. Unrelated identities remain. Original callee-preserved identities
are never regenerated by selected definitions.

A copy first requires the named current identity at its source, captures all
compatible proven equal identities there, kills destination aliases, and writes
those captured identities. Transfer scratch is unavailable to concurrent live
operands, secured indirect targets and bundle scratch. Target move effects kill
their declared working units; a recipe must establish its destination after those
kills. Scratch storage may be used only within its declared transfer point;
contents expire on leaving that point. Callee-save storage preserves only the
width promised by target facts. Frame checking later enforces physical save and
restore sequences against these obligations.

## Instruction and edge transitions

Entry seeds only checked ABI inputs and required original preserved contents,
then interprets explicit entry transfers. Parameter assignments are not entry
seeds. Before each instruction, interpret its transfers, then reconstruct events
in selected descriptor order:

1. Early uses, early clobbers, early definitions.
2. Late uses, late clobbers, late definitions.
3. After-instruction transfers, if this is a nonterminal instruction.

All uses in a phase observe the state before its clobbers/definitions; simultaneous
definitions require nonoverlapping destinations unless target facts explicitly
prove compatibility. Check fixed views, allowed banks/widths, reserved overlaps,
ABI component roles, ties, indirect-target protection and bundle scratch
independently. Destructive ties require equality of input/output locations at
their respective events. A live input must still have a surviving current copy
when next used. Calls read arguments and the secured indirect target before
caller kills and establish results afterwards. Securing a target before argument
marshalling is insufficient if a move or scratch later overwrites it. No-return
cells have no after-point, result availability or returning successor obligation.

On each edge occurrence independently:

1. Check and capture every argument identity at its assigned edge source before
   any edge transfer or rebinding.
2. Interpret the explicit ordered transfers. Check every successor parameter
   destination contains its captured argument's identity. A resolved swap must
   use declared scratch; sequential destructive copies cannot certify a cycle.
3. Remove **all** successor parameter identities from every location at once.
   This invalidates saved copies from the previous loop epoch, including a
   self-edge. Rebind captured arguments simultaneously at assigned destinations.
   Equal arguments may establish multiple parameter identities at one compatible
   destination; incompatible or overlapping destinations reject. Do not rename
   sequentially or erase an argument before another parameter captures it.

The specification oracle's `Rebind` models the ideal simultaneous relation in
step 3 using captured sources. Its separate copy-cycle fixtures model step 2.
The production checker must compare actual transfers with the captured relation;
it must never treat ideal rebinding as a missing physical move.

## Joins, convergence and deterministic rejection

Compute reachability from selected entry, retaining every edge occurrence.
Validate structural/type/resource/ABI/transfer legality for **all** blocks first.
Unreachable blocks acquire no availability facts and cannot manufacture dominance
or authorize a reachable use. Content checking is required on every reachable
path; sampling paths or native runs is insufficient.

At a reachable block entry, intersect identity sets location by location across
**all** incoming edge occurrences. Missing contents on one path yield unknown.
Include the fixed entry seed as an extra predecessor at entry, even on a backedge.
Initialize entry from that seed and other reachable block states to top.
Evaluate monotone transfers with unknown reads propagating unknown; defer use
failures until convergence. A diagnostic from an intermediate top state is never
acceptance or rejection. Empty contents do not make a reachable block unreachable.

Use deterministic rounds in selected block order, reading the previous round's
states, with edges in occurrence order. States only lose facts. If `B` is the
reachable block count, `L` the location count and `T` the identity count, there
are at most `B * L * T` strict state-fact removals. At most that many changing
rounds plus one stable round are needed. Each round has a finite ordered sweep of
selected events, edge events and declared transfers. Compute counts/products
with checked arithmetic; an unrepresentable bound is a structured capacity
failure, never wraparound, arbitrary timeout or optimistic acceptance.

After convergence, replay all reachable transitions with strict use checks.
Report failures in stable block, instruction/event and edge-occurrence order,
with selected site/origin, operand or transfer index and a fixed reason enum.
The same malformed draft must select the same first diagnostic. Check missing
coordinates in assignment order, storage in declaration order and transfers in
point/index order. A failure to converge within the finite bound is a checker
invariant failure, not a fallback. There is no provisional `CheckedPlacement`.

## Counterexample checkpoint

The owner-local tests pair rejected and corrected cases for destructive live
ties, caller clobbers/results, secured indirect targets, mixed-bank copy cycles,
duplicate successor occurrences, loop parameter epochs, loop definition epochs,
simultaneous swaps and partial-width preservation. A native owner test supplies
the real x86 resource catalog, RDI/R11 roles, byte/full RAX views, XMM and caller
footprints to the independent oracle. The second synthetic profile gives its
secured target a separate link-like role and partially preserves a floating
resource's low lane; no x86 register numbering or bank-wide preservation rule
enters the state interpreter. Representation tests use genuine selected
publication to reject a same-ID replacement and check unreachable scratch,
edge completeness, storage types, ABI slots and explicit move kinds.

These cases have unambiguous outcomes under this contract. Continue with an
independent production checker, retaining the test oracle as a separate reference.
If implementing a rule exposes a mismatch, amend this contract and the frozen
design before producing authority; do not weaken acceptance or silently add a
producer-specific exception. Register-resident positive placements, preservation
obligations and corruption of individual constraints remain required checker
tests. Baseline production and frame realization follow checked placement.
