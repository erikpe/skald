# Function-Value Compiler Contract

Status: frozen compiler contract; syntax AST, canonical resolved
`FunctionTypeId` metadata, exact ordinary reference nodes, and address-taken
metadata implemented behind a type-check gate. The source-visible contract is
[Capture-Free Function Values](../language/FUNCTION_VALUES.md),
the [status matrix](../language/STATUS.md) owns availability, and the active
[implementation roadmap](../roadmaps/FUNCTION_VALUES_ROADMAP.md) owns phase
ordering.

The completed frontend stages parse recursive closed signatures, intern them
bottom-up by exact modes and closed child types, and resolve accessible
internal top-level functions and ordinary static methods to exact reference
nodes. Resolution rejects excluded target families, records deterministic
address-taken metadata, and exposes both in dumps. Type checking remains the
phase-owned boundary that rejects all function-value programs before HIR.

The completed pipeline will represent one capture-free function value as one exact,
non-null internal callable address paired statically with a canonical complete
signature. It adds explicit semantic operations and verified indirect calls
without an environment, erased signature, runtime allocation, or runtime ABI
extension.

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

The implemented boundary currently admits ordinary top-level and ordinary
class static references. Closed-specialization references remain explicitly
gated until generic function terms can be substituted in FVI2. Function-valued
bindings and fields shadow declaration names as callees, but their indirect
calls also remain explicitly gated until stored HIR support.

Direct syntactic calls remain their existing direct or static call forms.
Lexically shadowing function-valued bindings and calls through arbitrary
function-typed expressions resolve to explicit indirect calls. Resolution
retains the callee expression rather than trying to infer a target from names
or argument count.

HIR carries the checked function reference and indirect-call distinction.
Function values use neutral scalar initialization, load, store, assignment,
field, static, parameter, and return operations. Implementation owners whose
current names say `Primitive` while already expressing trivial scalar work
must be renamed cohesively to `Scalar`; a function value must not masquerade
as a language primitive.

Indirect-call checking uses the canonical signature and the ordinary argument
and result planners. Alias modes, aggregate destinations, shared-owner
transfer, optionals, nested function-valued results, cleanup, and failure
behavior are not duplicated in a reduced callback checker.

## MIR representation and verification

MIR adds `MirType::Function(FunctionTypeId)`, an eight-byte scalar value, and a
callable-address rvalue naming one exact eligible `CallableId`. `MirCallTarget`
adds an indirect form containing the stabilized callee `ValueId` and canonical
function type. It carries no receiver.

Lowering evaluates and stores the callee once before lowering explicit
arguments. The existing call machinery then prepares arguments and results in
source order. Keeping the callee in a normal MIR value prevents later
argument lowering, trace instrumentation, or register allocation from losing
the selected address.

Verification proves:

- every function type ID names declared canonical metadata;
- every callable address names a defined eligible internal function or static
  method whose exact signature matches the function type;
- each indirect callee is defined before use and has the declared function
  type;
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
direct caller. Separate closed generic static methods remain separate target
and effect nodes. Dumps expose address-taken sets and their induced effect
edges deterministically.

## x86-64 representation and internal ABI

The initial target representation is one non-null eight-byte code pointer with
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

Syntax, resolved, HIR, MIR, static-effect, and assembly dumps expose canonical
types, exact targets, callee expressions, callable-address operations,
indirect targets, candidates, effect edges, and register-indirect calls in
stable identity order. Diagnostics distinguish malformed syntax, signature
differences, ineligible or inaccessible targets, invalid storage roles,
non-callable expressions, and excluded compositions.

Phase-private tests stay with syntax, resolution, specialization, type
checking, MIR, static lifecycle, and backend owners. Cross-phase determinism
uses compiler integration tests. Complete diagnostics, evaluation order,
ownership, panic, trace, assembly, and native observations use focused golden
tests. The implementation finishes with `make check`, `make msrv-check`,
documentation validation, and diff hygiene.
