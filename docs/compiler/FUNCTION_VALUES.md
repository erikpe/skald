# Function-Value Compiler Contract

Status: implemented compiler contract. Syntax AST, canonical resolved, HIR, and MIR
`FunctionTypeId` metadata, exact ordinary reference nodes, address-taken
metadata, trivial stored/callable values, completely checked indirect calls,
verified callable-address MIR, and x86-64 code-pointer realization are
implemented. The source-visible contract is
[Capture-Free Function Values](../language/FUNCTION_VALUES.md),
the [status matrix](../language/STATUS.md) owns availability. The
[implementation roadmap](../archive/FUNCTION_VALUES_ROADMAP.md) records phase
ordering and closure evidence.

The completed frontend stages parse recursive closed signatures, intern them
bottom-up by exact modes and closed child types, and resolve accessible
internal top-level functions and ordinary static methods to exact reference
nodes. Resolution rejects excluded target families and records deterministic
address-taken metadata. Type checking lowers canonical signature metadata,
exact references, scalar storage, copying, assignments, internal callable
transport, and receiverless indirect calls through the ordinary argument and
result planners into HIR. MIR lowering and verification then establish
target-independent callable addresses, callee order, provenance, and complete
call carriers. The x86-64 backend realizes exact position-independent symbol
addresses and receiverless register-indirect calls through the ordinary
internal ABI. Conservative whole-program static-effect expansion, retention,
exact trace attribution, and complete composition conformance are also
implemented.

The target-independent pipeline represents one capture-free function value as
one exact, non-null internal callable address paired statically with a canonical
complete signature. It adds explicit semantic operations and verified indirect
calls without an environment, erased signature, runtime allocation, or runtime
ABI extension.

## Phase ownership

The frozen pipeline boundary is:

```text
source function types and callable names
    -> canonical closed FunctionTypeId and exact CallableId
    -> typed function references and indirect calls in HIR
    -> callable-address values and typed indirect targets in MIR
    -> verified whole-program target/effect candidates
    -> one-word x86-64 code pointers and register-indirect calls
```

Syntax owns recursive type punctuation and spans. Resolution owns function
type interning, name selection, visibility, shadowing, and target eligibility.
Generic specialization closes parameter-bearing function terms before
ordinary resolved classes. Type checking owns exact compatibility, stored
value roles, callable boundaries, arguments, results, and indirect-call HIR.
MIR lowering owns evaluation order and concrete storage. Verification owns all
trusted target, signature, dataflow, argument/result, and pointer-origin
invariants. The backend owns representation and call instructions. Static
lifecycle analysis owns conservative indirect effect expansion.

## Canonical identities and semantic types

`FunctionTypeId` is a stable global semantic identity. Its interned key is the
ordered parameter modes and closed parameter types plus the closed result
type. Source spans, names, grouping, aliases, and display spelling are absent
from the key. Function type tables are explicit program metadata in every
phase that needs to verify or render the signature.

The generic template layer has a structural function term whose parameters
and result recursively contain template types. Dependency discovery,
substitution, requirement validation, and dumps traverse every child.
Specialization interns children before the complete closed signature and
never permits a template parameter or unclosed target into ordinary HIR.

`CallableId` remains distinct from `FunctionTypeId`. Many top-level functions
or static methods may share one type. Closed generic specializations retain
different `MethodId` values even when substitution produces equal signatures.

## Resolution and HIR

Resolved IR records an explicit function reference containing its exact
eligible `CallableId`, canonical function type, and source span. Only internal
top-level functions and ordinary or closed-specialization static methods may
form this node. Instance/virtual/interface methods, lifecycle and generated
bodies, externals, intrinsics, raw templates, and inaccessible declarations
are rejected before HIR.

The implemented boundary admits ordinary top-level functions and ordinary or
closed-specialization static methods. Template signatures close recursively,
and a specialized static reference records the exact generated `MethodId`
separately from its canonical signature. Function-valued bindings and fields
shadow declaration names as callees and resolve to explicit indirect-call
nodes.

Direct syntactic calls remain their existing direct or static call forms.
Lexically shadowing function-valued bindings and calls through arbitrary
function-typed expressions resolve to explicit indirect calls. Resolution
retains the callee expression rather than trying to infer a target from names
or argument count.

HIR carries canonical function-type tables and checked references with
their exact `CallableId` and `FunctionTypeId`. Function values use neutral
scalar initialization, load, store, assignment, field, static, parameter, and
return operations. Synthesized object copying records function fields as
scalar fields, and destruction plans add no cleanup step. Primitive aliases,
casts, comparisons, and diagnostics remain primitive-only.

Indirect-call HIR records the checked callee expression first, its exact
`FunctionTypeId`, ordinary checked arguments, exact result, and complete span.
It has no receiver carrier. The same node remains an object producer for
caller-owned class results, while arrays, optionals, shared owners, aliases,
and function-valued results retain their ordinary plans.

Indirect-call checking uses the canonical signature and the ordinary argument
and result planners. Alias modes, aggregate destinations, shared-owner
transfer, optionals, nested function-valued results, cleanup, and failure
behavior are not duplicated in a reduced callback checker.

## MIR representation and verification

MIR adds `MirType::Function(FunctionTypeId)`, a target-independent trivial
scalar value, and a callable-address rvalue naming one exact eligible
`CallableId`. `MirCallTarget` adds an indirect form containing the stabilized
callee `ValueId` and canonical function type. It carries no receiver.

Lowering evaluates the callee once into a normal MIR value before lowering
explicit arguments. When argument lowering introduces control flow, the
callee is stored in and reloaded from an ordinary scalar spill across the CFG.
The existing call machinery then prepares arguments and results in source
order. This makes the callee-before-arguments contract mechanically visible.

Verification proves:

- every function type ID names declared canonical metadata;
- every callable address names a defined eligible internal function or static
  method whose exact signature matches the function type;
- each indirect callee is defined before use, has the declared function type,
  and came from definitely initialized non-null function storage on every
  incoming path;
- indirect targets have no implicit receiver;
- arguments and results satisfy the existing complete internal signature,
  ownership, cleanup, and ordering contracts; and
- zero, integers, casts, malformed metadata, or unknown symbols cannot create
  an ordinary function value.

Verification never infers receivers or signatures from argument counts.
Mutations must cover unknown targets, mismatched signatures, wrong callee
types, missing definitions, receiver injection, corrupt argument/result
carriers, and arbitrary pointer construction.

## Whole-program effects and retention

Static-lifecycle analysis inventories callable-address operations by exact
`FunctionTypeId`, orders each target set by `CallableId`, and records the first
deterministic formation span for each retained target. Address formation and
copying remain effect-free.

Taking or copying an address has no static read, write, initialization, or
shutdown effect. An indirect call is conservatively expanded to every
address-taken eligible callable with the exact same `FunctionTypeId`. Static
lifecycle inference adds call edges to all such candidates and applies its
existing transitive analysis.

This candidate set is sound because initial values originate only from known
internal symbols. External pointers, casts, integers, reflection, closures,
and FFI callback injection are excluded. A later flow-sensitive narrowing may
reduce candidates but cannot change semantics or weaken static-lifetime
safety.

Every address-taken target remains live for emission even when it has no
direct caller. MIR verification rejects missing or ineligible target bodies,
and the static-lifecycle certificate independently verifies its candidate and
retention inventory against final MIR. Separate closed generic static methods
remain separate target and effect nodes. Dumps expose candidate sets as
explicit retention decisions and render their induced `IndirectCall` edges.

The implemented
[static-lifecycle certificate](PHASES_AND_IR.md#frozen-static-lifecycle-certificate-direction)
separates these responsibilities. Each MIR product derives its own
exact-signature candidates for conservative indirect-effect expansion, while
the compact lifecycle proof contains only normalized effects reachable from
lifecycle roots. Callable retention is an explicit responsibility of
whole-program reachability rather than a second meaning of lifecycle
certificate identity.

## x86-64 representation and internal ABI

X86-64 legality
verifies the complete MIR program, computes checked function-value layout and
ABI classification, and accepts verified callable addresses and indirect
targets.

The target representation is one non-null eight-byte code pointer with
eight-byte alignment and the System V integer ABI class. Materialization loads
the exact internal function or specialized static-method symbol address using
the backend's position-independent address operation.

Indirect call lowering reuses the verified internal signature and existing
call layout:

1. prepare any hidden aggregate result destination;
2. classify and prepare explicit arguments through the ordinary ABI planner;
3. preserve runtime-trace call-site attribution;
4. load the stabilized callee into the designated scratch register; and
5. emit the existing register-indirect call instruction.

Integer, floating, stack, alias, inline-object, array, optional, shared-owner,
aggregate-result, and function-result conventions remain unchanged. The
backend must retain exact collision-free symbols for every address-taken
specialized method.

## Runtime and interoperability boundary

Function values add no environment word, heap allocation, owner operation,
destructor, metadata header, runtime helper, panic reason, public C symbol, or
runtime compatibility change. Runtime ABI version 9 and
`ska_rt_abi_v9` remain unchanged.

External and intrinsic declarations are not addressable function values.
Their linkage and compiler-selected implementations are not encoded in
`FunctionTypeId`. A future interoperation feature may generate an ordinary
internal adapter thunk, but raw foreign pointers, calling-convention tags, C
callback export, and separate-compilation callable ABI are outside this
contract.

## Determinism and tests

Syntax, resolved, HIR, preliminary MIR, and final MIR dumps expose canonical
types, exact targets, callee expressions, callable-address operations, and
indirect targets in stable identity order. Assembly output exposes exact
symbol addresses and deterministic register-indirect calls. Static-effect
dumps expose exact-signature candidates, retained targets, induced edges, and
their transitive witnesses.
Diagnostics distinguish malformed syntax, signature differences,
ineligible or inaccessible targets, invalid storage roles, non-callable
expressions, and excluded compositions.

Phase-private tests stay with syntax, resolution, specialization, type
checking, MIR, static lifecycle, and backend owners. Cross-phase determinism
uses compiler integration tests. Complete diagnostics, evaluation order,
ownership, panic, trace, assembly, and native observations use focused golden
tests. The implementation finishes with `make check`, `make msrv-check`,
documentation validation, and diff hygiene.
