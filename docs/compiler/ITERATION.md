# General-Iteration Compiler Contract

Status: implemented compiler contract with syntax, canonical protocol identity,
nominal resolution, the complete loop-duration receiver matrix, and the
complete stored-value state/item matrix.
Resolution retains exact selected interface, requirement, item/state, local,
and loop identities. Type checking emits complete structured HIR for exact,
produced, checked, shared-backed, optional-derived, and array-backed read-only
receivers. Primitive, exact-class, inline-array, optional, shared-owner,
optional shared-owner, and optional-box-owner states and items lower through
their ordinary copy, transfer, and destruction plans to verified cyclic MIR
and native code. The
[language status matrix](../language/STATUS.md)
remains authoritative for implementation maturity.

This document owns the selected phase, lifetime, verification, target, and ABI
boundaries for the implemented
[general-iteration language contract](../language/ITERATION.md). It constrains
implementation shape without making target layout or internal Rust types part
of the language.

## Canonical interface identity

Request-local module resolution loads the exact `std::iter` module through the
ordinary standard-library provider. Resolution validates one public generic
interface named `Iterable` with exactly two parameters and the exact
`iter_state` and `iter_next` requirements from the language contract. The
validated declaration supplies template identities; each selected closed
application supplies its ordinary `InterfaceId` and requirement identities.
Lower phases do not recognize source spelling.

The compiler does not synthesize the declaration, accept a lookalike module,
or use structural method lookup. Missing, inaccessible, ambiguous, or malformed
canonical declarations stop before typed HIR with deterministic diagnostics.
The declaration remains ordinary dependency-free Skald source and participates
in normal module visibility and generic-interface specialization.

This canonical identity boundary is implemented. Successfully parsed `for-in`
keywords, explicit imports, and direct canonical-module compilation provide
requirement evidence. Module edges separately retain explicit-import spans and
typed compiler-dependency spans, so implicit acquisition creates no import
binding. Missing and ambiguous modules retain ordinary provider diagnostics,
malformed declarations use a focused resolution diagnostic, and valid closed
applications continue through the existing generic-interface specialization
coordinator.

## Syntax and resolution

Syntax retains a dedicated source-shaped `for-in` statement with delimiter,
binding, optional annotation, iterable-expression, body, and complete spans.
Recovery must preserve later statements and distinguish a missing contextual
`in` delimiter from an expression error. Generic-template body discovery must
recognize the same statement shape.

This syntax boundary is implemented, including deterministic dumps, logical
depth checking of the iterable, generic-source request scanning, and recovery.

Resolution allocates a callable-local `LoopId` in source order and a body-local
binding identity. The iterable expression resolves outside that binding's
scope; the body resolves with the binding and active loop identity. `break` and
`continue` reuse the existing nearest-loop resolution stack.

Resolved iteration retains the source components and selected protocol
evidence needed for type checking. Selection is over exact closed
`Iterable<Item, State>` claims or an already selected generic-bound
requirement. An explicit annotation is an exact candidate filter. Candidate
order must not affect acceptance, diagnostics, or dumps.

This resolution boundary is implemented. Exact candidates are enumerated from
direct, inherited, specialized-generic, and interface-view claims and sorted by
closed interface identity. A successful statement owns a source-ordered
`LoopId`, an exact `ResolvedLocal`, the resolved iterable, selected interface
and requirement IDs, `Item` and `State`, origin and source spans, and a resolved
body. Generic-bound selection is recorded structurally during template
checking and substituted to ordinary identities during specialization; the
closed concrete type is never searched again. Missing, ambiguous, mismatched,
structural-lookalike, and noncanonical cases remain deterministic resolution
errors.

## Typed HIR plan

Successful type checking emits a dedicated structured `HirForIn`. It retains:

- the loop, item-binding, selected closed interface, and exact requirement
  identities;
- exact `Item`, `State`, and `Item?` types;
- the typed iterable expression and a loop-duration read-only interface-view
  plan, including any owner, guard, or anchor;
- the selected receiver dispatch for `iter_state` and `iter_next`;
- state initialization, mutable state-alias, optional-result, payload-to-item,
  and cleanup plans;
- the typed body, source spans, and control-effect summary.

No unresolved generic parameter, protocol name lookup, candidate set, or
target-specific layout reaches HIR. Generic-class specialization substitutes
the definition-site selected interface and requirement identities rather than
selecting again.

This boundary is implemented for the complete receiver matrix.
`HirForIn` retains an ordinary or checked loop-duration receiver carrier,
source-independent stored-value lifecycle plans for the hidden state and fresh
item, exact call targets and access, a
mutable alias plan to the exact hidden state, and the canonical result
`OptionalTypeId` with its one-layer presence, unwrap, and destruction plans.
The item binding is checked as a read-only owning local while its body is
checked. An item must have the ordinary copy capability required to extract it
from the caller-owned optional result; state initialization can adopt a
produced non-copyable value. Unsupported storage categories and unavailable
ordinary copy operations are diagnosed before MIR with their existing
capability diagnostics.

The loop-duration receiver is distinct from a call-duration alias. Produced
storage, checked and optional guards, strong shared owners, optional-box roots,
and array backing anchors are promoted into its lexical cleanup bundle. It is
acquired once before `iter_state`, remains valid across every body execution,
and is released after state cleanup. A guarded inline optional root cannot be
replaced by the body; replaceable shared and array roots remain safe because
their anchors detach receiver lifetime from the source place.

## MIR lowering

HIR-to-MIR lowering expands `HirForIn` into ordinary target-independent
operations. There is no `MirForIn`, iterator opcode, target iterator primitive,
or runtime service.

This boundary is implemented for the complete receiver and stored-value
matrices. Lowering allocates the main regions before their edges, retains one
compiler-local state epoch, uses repeatable result and item epochs, and emits
only existing call, optional, class, array, shared-owner, lifetime, cleanup,
branch, and jump operations.

The generated CFG has these semantic regions:

```text
preheader: evaluate/acquire receiver -> iter_state -> own State
header:    iter_next(mut ref State) -> own Item? -> test outer presence
absent:    clean result -> clean State -> release receiver -> exit
present:   initialize fresh Item -> consume/clean result -> body
latch:     clean iteration scope -> header
exit:      continue after loop
```

Exact block factoring is implementation-private, but call count, evaluation
order, storage epochs, ownership transfer, and cleanup edges must match this
shape. Existing `LoopId` lowering contexts supply exit and latch targets for
both `while` and `for-in`. Their private cleanup contract records separate
retained depths for `break` and `continue`: `while` uses the same depth for
both, while iteration preserves its outer receiver/state scope only on
`continue`.

MIR uses ordinary interface-view acquisition, interface calls, mutable aliases,
optional construction/testing/payload extraction, stored-value initialization,
branches, jumps, storage begin/end, and cleanup instructions. `continue`
targets the iteration latch; `break` targets cleanup that destroys the state
and receiver resources exactly once. Return composes loop cleanup with existing
function-scope cleanup. Panic retains the existing non-unwinding boundary.

## Verification and deterministic evidence

Resolved and HIR dumps expose the source loop identity, binding, exact selected
interface application, `Item`, `State`, requirement identities, receiver plan,
and control effects. MIR dumps remain ordinary CFG and operation dumps; stable
comments may identify generated regions but cannot substitute for verifiable
ownership facts.

Type checking and final MIR verification together must establish:

- one receiver acquisition, one state initialization, and one state cleanup;
- one `iter_next` per reachable attempt and none after the absent edge;
- a mutable alias to exactly the live state and a read-only receiver;
- an exact `Item?` result and one-layer outer presence test;
- payload initialization into a fresh owning item before body entry;
- balanced storage epochs and exact cleanup on normal, continue, break, and
  return edges;
- valid owner, checked-view guard, shared anchor, and array-backing lifetimes;
- no unresolved protocol selection or generic term below HIR; and
- deterministic declaration, candidate, block, temporary, and diagnostic
  ordering.

Verifier mutation tests must reject wrong interface/requirement identities,
wrong state aliases or item types, calls after termination, missing or duplicate
cleanup, invalid loop targets, unbalanced storage, and insufficient receiver
anchors.

These boundaries have direct hardening evidence. Parser recovery retains the
following statement after every malformed header component. Resolution labels
the malformed canonical declaration or loop header as the primary cause and
labels candidate declarations or requirements as secondary evidence, with
candidate identities in canonical order. HIR construction rejects inconsistent
interface/requirement identities, receiver access, and `Item`/`State`/result
types before lowering. Final-MIR mutation tests cover call signatures, aliases,
nested optional layers, storage epochs, loop destinations, receiver owners,
checked-view guards, shared anchors, cleanup multiplicity, and post-termination
control flow through the ordinary verifier.

Determinism tests compare resolved, HIR, preliminary-MIR, planned-MIR,
final-MIR, and assembly output across processes while reversing source-file,
provider-module, and equivalent ambiguous-claim discovery order. Deep mixed
`while`/`for-in` construction additionally checks deterministic verified CFG
growth without adding an iteration-specific recursion path.

## Target and ABI boundary

Backends receive only verified ordinary MIR calls, optional operations,
lifetime operations, and cyclic CFG. Existing interface witness metadata and
internal calling conventions realize protocol calls. State and item layout use
their ordinary exact types. Backend loop handling remains insensitive to
source loop kind and lexical cleanup depth.

General iteration adds no public runtime symbol, metadata format, allocation
rule, external C calling convention, or ABI-version change. A library
implementation may allocate because its chosen `State` or method body does;
the loop mechanism itself requires no allocation. The current runtime marker
therefore remains unchanged.

## Implemented profile

The implemented profile includes ordinary exact, inherited, specialized generic, and
generic-bound selection; the complete accepted receiver and item families;
nested optional items; loop exits and cleanup; ordinary `Vec<T>` conformance;
deterministic diagnostics and dumps; verifier-negative evidence; and native
x86-64 observations.

The frozen [generic-range compiler contract](RANGES.md) separately defines
canonical `Range<T>`, `..`, primitive successor evidence, and one immediate
integer syntax-fusion plan over this implemented foundation. Primitive or
array intrinsic iterable conformance, generators, borrowed items, and broader
optimization guarantees remain separate work. The confirmed iteration
rationale is preserved in the
[design proposal](../archive/GENERAL_ITERATION_DESIGN_PROPOSAL.md), and tasks
are ordered in the
[implementation roadmap](../archive/GENERAL_ITERATION_ROADMAP.md).
