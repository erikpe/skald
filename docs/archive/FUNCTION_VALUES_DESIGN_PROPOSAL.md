# Function Values Design Proposal

Status: frozen design. FV1 through FV15 were confirmed together on 2026-08-15;
FV16's promotion procedure was then completed by publishing the living
language and compiler contracts and creating the implementation roadmap. The
implemented grammar and status matrix remain authoritative for current
compiler availability.

This proposal adds capture-free function values to Skald. A function value is
an always-valid, trivially copied reference to one exact internal top-level
function or static method. Its type records the complete parameter modes,
parameter types, and result type needed to call that target through Skald's
existing internal ABI. Function values do not capture state and are not
closures.

The design follows the useful boundary established by the sibling Niflheim
compiler while adapting it to Skald's canonical type identities, explicit
`ref` and `mut ref` parameter modes, inline object and array values, shared
ownership, closed generic-class specialization, verified MIR, and whole-program
static lifecycle analysis.

## Intended outcome

The initial function-value feature should provide:

- recursive function type syntax such as `fn(i64) -> bool` and
  `fn(fn(i64) -> bool, i64) -> bool`;
- exact parameter modes in function types, including `ref` and `mut ref`;
- values formed from accessible internal top-level functions and static
  methods;
- values formed from static methods on ordinary and closed generic-class
  specializations;
- storage in locals, value parameters and results, instance fields, and
  explicitly initialized static fields;
- use at every internal callable boundary, including ordinary, static,
  instance, virtual, and interface calls;
- calls through arbitrary function-typed expressions with deterministic
  evaluation and ordinary Skald argument, result, ownership, and cleanup
  behavior;
- contextual use of function types as closed generic-class arguments and in
  parameter-bearing template type terms;
- one-word, non-null code-pointer realization on the initial x86-64 target;
- explicit resolved, HIR, and MIR operations plus verifier coverage;
- sound indirect-call participation in static-effect analysis, symbol
  retention, liveness, and panic-trace attribution; and
- no runtime ABI extension or runtime-managed resource.

A representative source surface is:

```ska
fn increment(value: i64) -> i64 {
    return value + 1;
}

fn apply(callback: fn(i64) -> i64, value: i64) -> i64 {
    return callback(value);
}

class Identity<T> {
    static fn apply(value: T) -> T {
        return value;
    }

    static fn callback() -> fn(T) -> T {
        return Identity<T>::apply;
    }
}

class Hooks {
    callback: fn(i64) -> i64;
    static default_callback: fn(i64) -> i64 = increment;

    init(callback: fn(i64) -> i64) {
        self.callback = callback;
    }
}

fn use() -> i64 {
    var callback: fn(i64) -> i64 = Identity<i64>::apply;
    return apply(callback, 41);
}
```

## Current boundary and architectural evidence

Function values, closures, lambda literals, and calls through expression
values are currently unsupported. The implemented grammar already represents
a call as a postfix operation over a source expression, but resolution turns
eligible named calls into direct target identities and diagnoses top-level
functions or methods used as values. Resolved, HIR, and MIR types have no
function variant, and MIR call targets cover direct functions, static methods,
instance dispatch, and interface dispatch only.

The compiler nevertheless already contains most of the mechanisms the feature
should reuse:

- recursive source types and canonical bottom-up array and optional interning;
- exact callable declarations with value, read-only alias, and mutable alias
  parameter modes;
- ordinary argument checking for scalar values, inline objects, arrays,
  shared-owner transfer, recursive optionals, and aliases;
- caller-owned result destinations for inline object, array, and aggregate
  results plus dedicated shared-owner result transfer;
- closed generic-class specialization that gives each application an ordinary
  `ClassId`, substituted method declarations, and exact `MethodId` values
  before HIR;
- explicit MIR call targets and verification of arguments, results, receiver
  shape, ownership handoffs, and cleanup;
- x86-64 symbol-address loads and register-indirect call instructions already
  used by other backend responsibilities; and
- whole-program static-effect expansion across direct, virtual, and interface
  calls.

Niflheim demonstrates a viable capture-free surface: `fn(T...) -> R` types,
top-level and static-method references, function-typed locals, parameters,
results and fields, and code-pointer indirect calls using the ordinary target
ABI. Its implementation is evidence for the language boundary, not a phase
model to copy. In particular, Skald must not copy Niflheim's string-tagged
callable types, receiver inference from argument counts, or assumptions that
omit alias modes and deterministic ownership.

## Design principles

1. **A function value is not a closure.** It carries no receiver, capture
   environment, allocation, owner, or cleanup obligation.
2. **The signature is canonical and complete.** Parameter modes participate in
   type identity together with exact parameter and result types.
3. **Ordinary values are always valid.** A function value is non-null and has
   no uninitialized or moved-from source-visible state.
4. **Target identity and type identity are distinct.** Different functions may
   have the same function type, and different generic specializations remain
   different callable targets even when their signatures match.
5. **Indirect calls reuse ordinary calls.** They do not acquire a reduced
   primitive-only ABI, ownership model, or result profile.
6. **Specialization closes before execution.** A template method is never a
   runtime function value; a static method on one closed specialization is.
7. **Access is checked at formation.** A successfully formed value may be
   passed and called without re-performing member-name visibility checks.
8. **Source effects remain ordered.** The callee is evaluated exactly once
   before explicit arguments, which continue left to right.
9. **The first target representation stays small.** A one-word code pointer is
   sufficient for capture-free internal targets; future closures require a
   separate representation decision.
10. **Whole-program analyses remain sound.** Indirect calls must not become
    invisible to static lifecycle planning, verification, symbol retention,
    tracing, or optimization.

## Decision register

| ID | Decision | Proposed direction | State |
|---|---|---|---|
| [FV1](#fv1--function-type-syntax) | Type syntax | Add recursive `fn(...) -> ...` types with unnamed value, `ref`, and `mut ref` parameters | **Confirmed** |
| [FV2](#fv2--canonical-identity-and-compatibility) | Type identity | Intern exact signatures including parameter modes; use invariant equality and no adaptation | **Confirmed** |
| [FV3](#fv3--value-formation-and-eligibility) | Value sources | Permit accessible internal top-level functions and ordinary or closed-specialization static methods | **Confirmed** |
| [FV4](#fv4--closed-generic-composition) | Generics | Close parameter-bearing function types and static references during class specialization; permit contextual function type arguments | **Confirmed** |
| [FV5](#fv5--storage-initialization-copying-and-lifecycle) | Stored values | Treat function values as non-null trivial scalars; require explicit initialization where zero is not valid | **Confirmed** |
| [FV6](#fv6--internal-callable-boundaries) | Callable composition | Permit function values across every internal direct, static, instance, virtual, and interface boundary | **Confirmed** |
| [FV7](#fv7--indirect-call-semantics-and-evaluation-order) | Calls | Call arbitrary function-typed expressions; evaluate the callee once before left-to-right arguments | **Confirmed** |
| [FV8](#fv8--name-resolution-shadowing-and-access) | Resolution and access | Preserve direct calls, prefer shadowing values where applicable, and check visibility when forming a reference | **Confirmed** |
| [FV9](#fv9--semantic-ir-and-canonical-type-tables) | Frontend IR | Add explicit function references, indirect calls, and canonical function tables to resolved IR and HIR | **Confirmed** |
| [FV10](#fv10--mir-representation-and-verification) | MIR | Add callable-address rvalues and typed indirect call targets with complete verifier checks | **Confirmed** |
| [FV11](#fv11--target-representation-and-internal-abi) | Backend and ABI | Represent one non-null code pointer, classify it as an integer scalar, and reuse the complete internal call ABI | **Confirmed** |
| [FV12](#fv12--static-lifecycle-effects-and-symbol-retention) | Whole-program effects | Expand indirect calls to address-taken exact-signature candidates and retain every referenced symbol | **Confirmed** |
| [FV13](#fv13--modules-linkage-and-interoperability) | Modules and linkage | Preserve ordinary qualification and visibility; exclude external and intrinsic references initially | **Confirmed** |
| [FV14](#fv14--diagnostics-dumps-and-testing) | Quality | Make signatures, target identities, evaluation, ownership, effects, and exclusions observable and deterministic | **Confirmed** |
| [FV15](#fv15--initial-exclusions-and-future-compatibility) | Feature boundary | Exclude closures, bound methods, optional/array function values, casts, equality, FFI pointers, and callback-slot aliases | **Confirmed** |
| [FV16](#fv16--promotion-and-roadmap-boundary) | Delivery | Confirm the register, promote living contracts, then create a PR-sized implementation roadmap | **Completed** |

## FV1 — Function type syntax

The proposed type shape is:

```text
function-type           = "fn" "(" [function-type-parameter
                          {"," function-type-parameter}] ")"
                          "->" storage-type
function-type-parameter = storage-type
                        | "ref" storage-type
                        | "mut" "ref" storage-type
```

The production is recursive through `storage-type`, permitting higher-order
parameters and results:

```ska
fn() -> unit
fn(i64, f64) -> bool
fn(ref Item, mut ref Counter) -> unit
fn(fn(i64) -> bool, i64) -> bool
fn(i64) -> fn(bool) -> unit
```

Function-type parameters have no names because the type describes only the
call contract. `ref` and `mut ref` are binding modes, just as they are in
callable declarations; they are not reference type constructors. A result has
no binding mode because Skald does not return aliases.

Function types are accepted in ordinary stored type positions subject to the
role restrictions in this proposal. Grouping may disambiguate a type but does
not create another identity. A function returning an array or optional is
different from an array or optional containing a function value:

```text
fn() -> i64[]       function returning an array; permitted
fn() -> i64?        function returning an optional; permitted
(fn() -> i64)[]     array of function values; initially rejected
(fn() -> i64)?      optional function value; initially rejected
```

Malformed parameter modes, missing parentheses or arrows, trailing unexpected
tokens, and excessive nesting receive syntax-owned diagnostics with exact
spans.

## FV2 — Canonical identity and compatibility

Resolution interns every closed function signature into one
`FunctionTypeId`. Its canonical key is conceptually:

```text
FunctionTypeKey {
    parameters: [FunctionParameterType {
        mode: Value | ReadOnlyAlias | MutableAlias,
        type: ResolvedTypeKind,
    }],
    result: ResolvedTypeKind,
}
```

Source spans, grouping, module aliases, optional shorthand, and display text
do not participate in equality. Nested arrays, optionals, shared targets,
closed generic classes, and function types already carry their canonical
identities when used in the key.

Function types are invariant and arity-exact. Assignment, initialization,
return, field storage, generic substitution, and call selection require the
same ordered parameter modes, exact parameter types, and exact result type.
There is no parameter contravariance, result covariance, implicit receiver
binding, currying, argument dropping, numeric adaptation, or automatic wrapper
generation.

These are distinct:

```text
fn(Item) -> unit
fn(ref Item) -> unit
fn(mut ref Item) -> unit
fn(Base) -> Derived
fn(Derived) -> Base
```

Canonical type tables belong to resolved, HIR, and MIR program products as
needed by their trust boundaries. Later phases preserve `FunctionTypeId`
rather than rebuilding a signature from display names.

## FV3 — Value formation and eligibility

The initial feature forms function values from:

- an accessible internal top-level function, including a qualified imported
  function; and
- an accessible internal static method selected through an ordinary class or
  a closed generic-class application.

Examples are:

```ska
var parse: fn(Str) -> i64 = parse_value;
var imported: fn(i64) -> bool = util::accept;
var ordinary: fn(i64) -> i64 = Math.increment;
var specialized: fn(i64) -> i64 = Identity<i64>::apply;
```

The selected declaration determines both the exact `CallableId` and the
canonical function type. An expected type does not select a different
overload or adapt the signature. Skald has no top-level or ordinary-method
overloading to resolve through a function-value context.

The selected entry function may be used as a value after ordinary entry-point
validation. Recursive or repeated invocation is not special to function
values.

The following are not eligible initial targets:

- instance methods, whether described as bound or unbound;
- virtual or interface method selections;
- initializers, lifecycle methods, generated static initializer bodies, or the
  generated program coordinator;
- external declarations;
- intrinsic declarations; and
- a raw generic template or an unclosed method reference.

A supported source expression always yields a non-null code address. No
source literal, cast, default value, or zero initialization can invent a
function value.

## FV4 — Closed generic composition

Generic class specialization applies to function types and static references
before ordinary HIR. The template type layer gains a structural function node
whose parameter and result terms may contain the class's type parameters:

```text
Function {
    parameters: [(mode, ResolvedTemplateType)],
    result: ResolvedTemplateType,
}
```

Parameter-dependency checks recurse through every parameter and the result.
Specialization substitutes each nested type term, closes nested generic
applications, interns compound children, and finally interns the resulting
`FunctionTypeId`.

For example:

```ska
class Identity<T> {
    static fn apply(value: T) -> T { return value; }

    static fn callback() -> fn(T) -> T {
        return Identity<T>::apply;
    }
}
```

produces ordinary closed artifacts such as:

```text
Identity<i64>::apply
    target = MethodId(ClassId(Identity<i64>), apply)
    type   = fn(i64) -> i64

Identity<bool>::apply
    target = MethodId(ClassId(Identity<bool>), apply)
    type   = fn(bool) -> bool
```

The targets differ because their specialized `ClassId` values differ. If a
static method's signature does not mention `T`, separate specializations still
produce separate `MethodId` and code-address identities even though they share
one `FunctionTypeId`.

A generic static selection used as a value requests the closed specialization
just as construction, static field selection, or a direct static call does.
`Identity<T>::apply` inside a template is retained in the template body and
becomes an exact reference only while specializing that body.

Function types may be explicit generic arguments. They remain subject to the
generic feature's contextual requirements rather than a global whitelist:

```ska
Holder<fn(i64) -> i64>
```

The function type satisfies ordinary field/static storage with an explicit
static initializer, value parameter/result, trivial copy, assignment, and
destruction requirements. It initially fails requirements that specifically
demand an optional payload, array element, shared target, or alias target.
Consequently an unused marker parameter may accept a function type, a simple
`Holder<T> { value: T; }` may store one, and a template whose representation is
`T?[]` rejects it at the precise optional or array requirement.

Generic top-level functions, method-level type parameters, and runtime generic
callable identities remain outside this proposal.

## FV5 — Storage, initialization, copying, and lifecycle

An ordinary function value is an always-present stored scalar. It supports:

- explicitly initialized locals and later reassignment;
- value parameters and value results;
- instance fields initialized by ordinary class initialization;
- explicitly initialized static fields;
- direct and synthesized field copying and assignment; and
- contextual generic storage satisfying those same capabilities.

Copying or assigning a function value copies its code-pointer bits. It does
not invoke a copy constructor, retain an owner, clone code, or create an
environment. Destruction is a no-op. Explicit object-copy syntax does not
apply to a function value.

Skald never exposes an invalid ordinary value. Locals continue to require
initializers, class fields must be established on every successful initializer
path, and function-valued statics must have explicit declaration initializers:

```ska
class Hooks {
    static valid: fn() -> unit = default_hook;
    static invalid: fn() -> unit; // rejected
}
```

Initializer-free static storage is zero-filled, but zero is not a complete
function value. The compiler rejects that declaration at the stored type
instead of silently creating a nullable pointer. An explicit static
initializer directly establishes the first live value under the existing
eager initialization and publication rules.

Function-valued fields participate in synthesized class copy construction and
copy assignment as trivial scalar fields and add no cleanup step. Static
specializations retain independent function-valued storage and initializer
effects per closed class.

## FV6 — Internal callable boundaries

Function values are valid value parameters and results on every internal
callable family whose ordinary value types are checked and lowered by Skald:

- top-level functions;
- static methods;
- instance methods;
- virtual methods and overrides;
- interface requirements and implementations;
- initializers where a value parameter is otherwise valid; and
- callable declarations generated for closed generic classes.

This does not make an instance, virtual, or interface method selection into a
function value. It permits those calls to transport a function value as one
ordinary scalar argument or result:

```ska
interface Runner {
    fn run(callback: fn(i64) -> i64, value: i64) -> i64;
}
```

The complete callable declaration validators apply. Override and interface
conformance compare the canonical function types exactly. External callable
signatures remain restricted by the external ABI contract and cannot contain
function values initially.

A function signature may itself accept inline objects, arrays, shared owners,
optionals, and aliases or return any supported internal result. Indirect calls
must preserve the existing type-specific argument construction, owner
transfer, hidden caller-owned result destination, result securing, cleanup,
and failure behavior. The feature is not limited to primitive callbacks.

## FV7 — Indirect call semantics and evaluation order

Every function-typed expression is callable with ordinary argument syntax:

```ska
callback(value)
holder.callback(value)
Hooks.callback(value)
choose_callback()(value)
factory.produce().callback(value)
```

The callee expression evaluates exactly once before every explicit argument.
Its code pointer is secured in compiler-owned temporary storage. Arguments
then evaluate exactly once from left to right. The indirect call runs only
after the callee and every argument complete successfully.

If callee evaluation terminates, no argument runs. If a later argument
terminates, the call does not run and every already completed temporary follows
the existing abrupt-termination boundary. A successful result is secured or
transferred before enclosing full-expression cleanup, exactly as for the
corresponding direct call.

Direct source calls retain direct targets when their callee syntax names an
eligible top-level function or static method. This is an IR and code-generation
distinction, not a source-visible difference. Calls through bindings, fields,
static fields, returned function values, and other function-typed expressions
are indirect.

The implemented cast ambiguity remains unchanged initially. `(f)(argument)`
continues to parse as an object-cast candidate rather than grouped callable
syntax. `f(argument)` and other unambiguous postfix chains remain available.
Changing that precedence belongs to a separate cast and grammar decision.

Only ordinary argument lists apply to an indirect call. Contextual `copy`
construction arguments do not turn a function value into a constructor.

## FV8 — Name resolution, shadowing, and access

Using an internal top-level function or eligible static method in value
position resolves to an explicit function reference rather than the current
"used as a value" diagnostic. Class and module qualification follow existing
declaration lookup rules and produce the already selected `FunctionId` or
`MethodId`.

A direct named call remains direct unless lexical value lookup shadows that
name. For example:

```ska
fn transform(value: i64) -> i64 { return value; }

fn apply(transform: fn(i64) -> i64, value: i64) -> i64 {
    return transform(value); // calls the parameter indirectly
}
```

Function-valued instance and static fields are callable. A field is selected
and evaluated under its ordinary receiver or static-place rules before the
call. Wrong-kind values retain targeted diagnostics rather than falling
through to a misleading unknown-function error.

Declaration and member visibility are checked when forming the reference. A
private static method may be captured only from a context that can name it.
Once validly formed, the value may be passed or returned and later invoked;
the indirect call does not contain a member name and does not repeat privacy
checking. This behavior treats the function value as a capability without
making the private declaration directly nameable elsewhere.

## FV9 — Semantic IR and canonical type tables

The syntax AST gains a recursive function-type node retaining parameter-mode,
child-type, punctuation, and complete spans. Resolution converts closed
signatures into `FunctionTypeId` and retains parameter-bearing equivalents in
the generic template layer.

Resolved expression IR gains explicit operations conceptually equivalent to:

```text
ResolvedFunctionReference {
    callable: CallableId,
    function_type: FunctionTypeId,
    span: Span,
}

ResolvedIndirectCall {
    callee: ResolvedExpression,
    function_type: FunctionTypeId,
    arguments: [ResolvedExpression],
    span: Span,
}
```

Only a top-level `FunctionId` or eligible static `MethodId` may inhabit the
reference node. Bound receivers are not represented and later rejected; they
must be rejected before constructing this node.

HIR retains the same semantic distinction with checked arguments and exact
result type. Direct, static, method, virtual, and interface call forms remain
unchanged. Indirect calls add no receiver carrier and retain the checked
`FunctionTypeId` explicitly or through the exact callee type.

Function values use neutral scalar storage, initialization, assignment, field,
parameter, and return machinery. Existing IR names that say `Primitive` while
already meaning any trivial scalar should be renamed to `Scalar` as part of
the owning phase change rather than teaching a function pointer to masquerade
as a language primitive.

Phase facades re-export the new identities and nodes while keeping interning,
selection, checking, and lowering implementation files private and cohesive.

## FV10 — MIR representation and verification

MIR gains `MirType::Function(FunctionTypeId)` as an eight-byte scalar type and
an rvalue that materializes one exact callable address:

```text
CallableAddress {
    target: CallableId,
}
```

`MirCallTarget` gains an indirect form containing the stabilized callee
`ValueId`. It has no receiver:

```text
Indirect {
    callee: ValueId,
    function_type: FunctionTypeId,
}
```

HIR-to-MIR lowering evaluates the callee first and assigns it a normal
stack-backed MIR value before lowering explicit arguments. Existing call
lowering then prepares arguments and results from the canonical signature.
Keeping the target in a `ValueId` prevents later argument lowering, runtime
trace recording, or register use from losing the code pointer.

Verification proves at least:

- every function type ID names declared canonical metadata;
- every callable-address target exists and is an eligible internal top-level
  function or static method;
- the target's exact declared parameter modes, parameter types, and result
  match the function type;
- every indirect callee value is defined before use and has the declared
  `MirType::Function`;
- an indirect call carries no implicit receiver;
- arguments satisfy the same value, alias, ownership, cleanup, and ordering
  rules as the corresponding internal signature;
- scalar, caller-owned aggregate, shared-owner, optional, and function-valued
  results use the required result carrier; and
- ordinary function values cannot be synthesized from zero or arbitrary
  integers.

The verifier must not infer a receiver by comparing argument counts. Call
shape and signature are explicit trusted inputs.

## FV11 — Target representation and internal ABI

The initial x86-64 representation is one non-null machine-word code pointer.
A function value has size and alignment eight and uses the SysV integer ABI
class. It contains the address of the exact emitted internal function or
specialized static-method entry symbol.

Materialization uses the backend's existing position-independent symbol-address
instruction and stores the result like another scalar. Indirect call lowering:

1. obtains the verified internal signature;
2. prepares hidden result destinations and explicit arguments through the
   ordinary call layout;
3. records panic-trace call-site attribution without losing the secured
   target;
4. loads the callee value into the designated call scratch register; and
5. emits the existing register-indirect call instruction.

Integer, floating, stack-passed, alias, and ownership arguments retain their
ordinary classification. Inline object, array, and aggregate results retain
their caller-owned destination. Shared and optional-shared results retain the
existing owner-transfer result convention. A function-valued result is one
integer-class scalar.

There is no hidden environment argument, capture record, allocation, retain,
release, destructor, runtime call, header field, metadata record, or runtime
ABI version change. Private symbol spelling remains a backend concern, while
each distinct specialized `MethodId` must resolve to its exact collision-free
symbol.

The one-word representation is an initial internal compiler contract, not a
promise that future closures use the same source type or ABI. A later closure
design must decide explicitly whether callable types remain code pointers,
gain an environment-bearing representation, or introduce another type family.

## FV12 — Static lifecycle effects and symbol retention

Forming or copying a function reference has no static read, write,
initialization, or shutdown effect. Calling the value may execute arbitrary
effects of its target and must therefore participate in whole-program static
lifecycle analysis.

The sound initial expansion for one indirect call is every address-taken
eligible internal callable whose exact `FunctionTypeId` matches the callee.
The static-effect graph adds call edges from the caller to each such target and
then applies the existing transitive effect analysis. Restricting the set to
address-taken targets is safe while values can originate only from compiler
known internal symbols and cannot enter from FFI, casts, integers, reflection,
or closures.

Closed generic static methods participate by their exact specialized
`MethodId`. Two specializations with the same function type are both possible
targets if both addresses are taken, and their class-owned static effects stay
independent.

Every address-taken target must retain an emitted body and symbol even if no
direct call references it. Liveness, reachability, optimization, assembly
ordering, and linker input must not discard it. A later flow-sensitive
function-value analysis may narrow candidate sets but cannot change observable
semantics or weaken lifecycle safety.

## FV13 — Modules, linkage, and interoperability

Top-level references use the existing local, selective-import, direct module
binding, qualification, and visibility rules. Import aliases affect source
lookup and diagnostics but not callable or function-type identity. Function
values may cross internal module boundaries because the whole-program compiler
already assigns exact declaration identities and emits the required private
symbols.

Static method references use ordinary class-member selection and
declaring-class privacy. A generic static selection requests and names the
closed specialization through the existing application-site and
definition-site rules.

External functions are excluded even when their source declaration resembles
an eligible function type. External linkage, C-compatible restrictions, and
result normalization are not encoded by the initial `FunctionTypeId` and must
not be recovered after the target identity has been erased into a value.
Intrinsic functions are likewise excluded because their implementation may be
compiler-selected rather than an addressable internal body.

A later interop design may generate an internal adapter thunk whose exact
Skald signature and body perform the foreign call. The function value would
then point to that ordinary internal thunk rather than directly carry foreign
linkage. Raw pointer import/export, source-visible calling conventions, and C
callback registration remain outside this proposal.

## FV14 — Diagnostics, dumps, and testing

Diagnostics should distinguish:

- malformed function type syntax;
- exact signature mismatch, including the first differing parameter mode;
- unsupported instance or interface method references;
- external or intrinsic declarations used as values;
- raw or failed generic specialization references;
- inaccessible top-level functions or private static methods;
- non-callable expression calls;
- invalid function-valued array, optional, shared-target, or callback-slot
  alias composition;
- initializer-free function-valued statics; and
- unsupported casts, equality, or `copy` construction involving function
  values.

Display names use canonical source-facing syntax such as
`fn(ref Item, i64) -> shared Result`. Diagnostics name the selected declaration
and its declaration span where useful. Generic failures retain the closed
application and the template type or reference that caused substitution.

Deterministic dumps should expose:

- source-shaped function type parameters and spans;
- canonical `FunctionTypeId` tables and nested signatures;
- template function terms and their closed substitutions;
- exact `CallableId` values for function references;
- resolved and HIR indirect callee expressions;
- MIR callable-address rvalues and indirect targets;
- address-taken candidate sets and static-effect edges; and
- target symbol-address and register-indirect call instructions.

Focused coverage should include at least:

- parser success and recovery for zero, multiple, alias-mode, nested parameter,
  and nested result signatures;
- array/optional containment rejection while permitting functions that return
  those types;
- top-level, imported, public, private, ordinary static, and generic static
  references;
- lexical shadowing of a same-named top-level function;
- exact signature mismatch across arity, mode, parameter, result, nested
  signature, class identity, and generic specialization;
- local, parameter, result, instance-field, reassignment, explicit-static, and
  generic-holder storage;
- rejection of initializer-free statics and unsupported target families;
- indirect calls through bindings, fields, static fields, returned values,
  chained produced expressions, virtual boundaries, and interface boundaries;
- callee-before-argument, left-to-right, exactly-once, failure suppression,
  result securing, and reverse cleanup observations;
- mixed integer and floating registers, stack overflow arguments, alias modes,
  inline object and array values/results, shared-owner transfer, optionals,
  function-valued results, and panic propagation;
- separate targets for closed generic specializations with changed signatures
  and for same-signature methods with different `MethodId` values;
- static-initializer dependencies reachable only through indirect calls;
- MIR mutations covering unknown targets, mismatched signatures, wrong callee
  types, missing definitions, implicit receivers, corrupt arguments/results,
  and arbitrary pointer construction;
- address-taken symbol retention and expected x86-64 `call` through a register;
  and
- deterministic syntax, resolved, HIR, MIR, diagnostics, assembly, and native
  results across repeated compilation.

Phase-private unit tests remain beside their owners. Cross-phase public tests
belong in the compiler integration suite. Complete source-to-diagnostic,
assembly, panic, lifecycle, and native observations belong in focused golden
tests. The implementation roadmap should finish with the repository's
documented `make check` gate and the supported MSRV gate for Rust changes.

## FV15 — Initial exclusions and future compatibility

The initial feature does not include:

- lambda literals, nested function declarations, captured-variable closures,
  or capture inference;
- bound or unbound instance method values;
- virtual-family or interface-requirement method values;
- callable class or interface objects;
- external, intrinsic, runtime, initializer, lifecycle, or generated-body
  addresses;
- nullable ordinary function values or a null function literal;
- optional function values;
- arrays whose element type is a function value;
- `shared` ownership of a function value;
- `ref` or `mut ref` parameters whose target is the variable slot holding a
  function value;
- casts to or from a function type;
- equality, ordering, hashing, formatting, byte conversion, reflection, or
  serialization of function values;
- explicit object-copy construction of function values;
- function overloading selected from an expected function type;
- generic top-level functions, method-level type parameters, or runtime
  generic callable values;
- foreign calling-convention annotations, raw address import/export, C
  callbacks, or stable separate-compilation ABI; or
- a promise that later closures use the same representation or are implicitly
  compatible with capture-free function types.

These exclusions leave a complete higher-order internal programming feature:
function values can be named, stored, copied, selected, transported through
all internal callable families, returned, composed with closed generic-class
specialization, and called with the full existing ownership and ABI profile.
The exclusions avoid prematurely coupling that feature to nullable containers,
array lifecycle, aliasing of callback slots, foreign linkage, or closure
environments.

Optional and array support should be designed as ordinary type composition,
not special callback containers. Optional function values must choose and
verify an absence representation and checked call path. Function-valued arrays
must define default-construction eligibility and integrate scalar element
load, store, list construction, slicing, copying, and replacement. Neither is
required to validate the core representation and call boundary.

## FV16 — Promotion and roadmap boundary

FV1 through FV15 were reviewed and confirmed as one coherent contract because
syntax, canonical signature identity, generic substitution, non-nullability,
target eligibility, internal ABI, and whole-program effects depend on one
another.

Promotion completed the following actions:

- add the frozen source semantics to focused living language documentation;
- publish the frozen grammar extension while retaining the current accepted
  grammar and `(f)(argument)` cast precedence until implementation;
- update the status matrix from open question to frozen design;
- add canonical function-type, IR, verifier, target, static-effect, and
  unchanged runtime ABI boundaries to compiler documentation;
- archive this proposal as the historical decision record; and
- create an active implementation roadmap with PR-sized tasks and objective
  exit criteria.

The implementation roadmap orders work by stable responsibility:

1. source syntax, `FunctionTypeId`, closed ordinary types, dumps, and
   diagnostics;
2. the eligible ordinary resolved-reference form, access, shadowing, and
   address-taken metadata;
3. template function terms, contextual generic capabilities, substitution,
   and closed-specialization references using that resolved form;
4. trivial scalar storage, callable transport, initialization, and lifecycle
   integration;
5. indirect-call resolution and full HIR argument/result checking;
6. MIR lowering, callee-before-argument stabilization, and verification;
7. x86-64 code-pointer realization and indirect call emission;
8. static-effect expansion, symbol retention, and trace attribution; and
9. complete documentation publication, negative coverage, generic
   composition, ownership/ABI hardening, deterministic goldens, and repository
   gates.

That roadmap may split a responsibility further to keep one reviewable purpose
per change, but it should not reopen the confirmed language or representation
decisions. New closure, optional, array, alias-slot, or interop work should be
recorded separately rather than expanding the initial implementation scope.
