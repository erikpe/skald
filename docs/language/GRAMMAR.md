# Implemented Skald Grammar

Status: authoritative grammar for the source syntax accepted by the current
compiler. [Feature status](STATUS.md) determines whether the syntactic forms
below have a complete semantic implementation.

This document defines tokens, concrete source shape, precedence,
associativity, and syntax-error boundaries. It does not define name lookup,
types, ownership, evaluation, lifecycle behavior, ABI, or lowering.

The frozen module syntax is deliberately absent from this complete-compiler
grammar until the whole-program module pipeline is implemented. Its exact
extension is owned by
[Modules and Foreign Interoperation](MODULES_AND_INTEROP.md#import-syntax);
the lexer and parser currently recognize it only as a phase-local,
source-shaped representation. `import`, `from`, `as`, and `public` remain
contextual identifiers, and the single-file semantic adapter rejects imports
and qualified `::` uses with a structured unsupported-module diagnostic. The
inactive whole-program graph resolver consumes parsed imports for reachability,
builds direct default or aliased module bindings, and resolves qualified source
uses. Selective imports remain reachability-only, and the supported driver
does not yet expose the whole-program module pipeline.

## Notation

The grammar uses this EBNF notation:

- `"text"` is a literal token;
- `name` is a nonterminal;
- `A | B` selects one alternative;
- `[A]` makes `A` optional;
- `{A}` repeats `A` zero or more times;
- `(A)` groups grammar terms.

Whitespace and comments may separate tokens unless doing so would split one
token. An explicit `EOF` denotes the end of the source file.

## Source text and trivia

Source files are UTF-8. Identifiers and whitespace currently use deliberately
restricted ASCII rules:

```text
ascii-whitespace     = " " | "\t" | "\r" | "\n"
line-comment         = "//" { source-character-except-newline }
trivia               = ascii-whitespace | line-comment

identifier-start     = "A".."Z" | "a".."z" | "_"
identifier-continue  = identifier-start | "0".."9"
identifier           = identifier-start { identifier-continue }
```

A line comment ends immediately before a newline or at `EOF`. Block comments
are not recognized. Non-ASCII characters are valid UTF-8 source characters
but are not valid identifier characters.

## Keywords and contextual words

The reserved keywords are:

```text
class self mut ref
fn extern var return
i64 u64 u8 f64 bool unit
true false
if elif else
none
```

`init`, `copy`, `assign`, and `destroy` are ordinary identifiers except in
their constructor or direct class-member forms:

- `init` followed by a parameter list begins an initializer;
- `copy` followed by a parameter list begins a copy constructor;
- `copy` immediately after the opening parenthesis of a class construction
  selects explicit copy construction;
- `assign` followed by a parameter list begins copy assignment;
- `destroy` followed directly by a block begins destruction.

They remain available as field names, method names, parameter names, local
names, and top-level function names. For example, `copy: i64;` is a field,
`fn copy() -> unit {}` is a method, and `T(copy)` passes a binding named
`copy` to an ordinary initializer.

`extends` is contextually recognized only after a class name. `super` followed
by a call argument list is contextually recognized as a dedicated statement;
resolution restricts it to the first statement of a derived ordinary
initializer. Both spellings remain ordinary identifiers outside those shapes.
`Obj` is contextually recognized as the universal object-view type in
alias-parameter and type-operation target positions; it remains an ordinary
identifier elsewhere except that it cannot name a top-level declaration.
`virtual` and `override` are contextually recognized only as method modifiers.
`implements`, `interface`, and `is` are likewise contextual in the exact forms
below. `some` is contextual only after `is`; `none` is reserved by the lexer
and forms either an absent expression or the target of a presence test.
`shared` is contextual before an object or array target in stored and result
types and inside a cast target. `new` is contextual before a class allocation
or an array construction. Both remain ordinary identifiers elsewhere.

## Punctuation

The complete punctuation and operator token set is:

```text
( ) { } [ ] , : ; . -> + - * = ? !
```

There are no string, character, comparison, or division tokens in the
implemented grammar.

## Literals

```text
decimal-digits  = digit { digit }
digit           = "0".."9"

i64-literal     = decimal-digits
u64-literal     = decimal-digits "u"
u8-literal      = decimal-digits "u8"

exponent        = ("e" | "E") ["+" | "-"] decimal-digits
f64-literal     = decimal-digits "." decimal-digits [exponent]
                | decimal-digits exponent

bool-literal    = "true" | "false"
numeric-literal = i64-literal | u64-literal | u8-literal | f64-literal
literal         = numeric-literal | bool-literal
```

Leading `-` is a separate unary token. A decimal point requires digits on both
sides, and an exponent requires at least one digit. Numeric-looking suffixes or
tails not present above make the complete numeric-looking spelling invalid;
examples include `1_000`, `0xff`, `1.`, `1e+`, `1.2.3`, and `42u64`. A leading
dot is punctuation, so `.5` tokenizes as `.` followed by `5` and is not a
floating literal. Literal ranges and value interpretation are semantic rules,
not grammar.

## Compilation unit and declarations

```text
compilation-unit              = { top-level-declaration } EOF

top-level-declaration         = function-definition
                              | external-function-declaration
                              | class-declaration
                              | interface-declaration

function-definition           = "fn" identifier parameter-list
                                "->" result-type block

external-function-declaration = "extern" "fn" identifier parameter-list
                                "->" result-type ";"

parameter-list                = "(" [parameter {"," parameter}] ")"
parameter                     = value-parameter | alias-parameter
value-parameter               = identifier ":" storage-type
alias-parameter               = ["mut"] "ref" identifier ":" alias-target
alias-target                  = identifier | identifier "?" | "Obj"

primitive-type                = "i64" | "u64" | "u8" | "f64" | "bool"
named-type                    = identifier
shared-target                 = identifier | "Obj"
shared-type                   = "shared" shared-target
inline-optional-type          = (primitive-type | named-type) "?"
optional-shared-type          = "shared" "?" shared-target
type-primary                  = primitive-type | named-type | "unit"
                              | "(" storage-type ")"
postfix-array-type            = type-primary ["?"] "[" "]" {"[" "]"}
array-storage-type            = ["shared" ["?"]] postfix-array-type
storage-type                  = primitive-type | named-type | shared-type
                              | inline-optional-type | optional-shared-type
                              | array-storage-type
result-type                   = storage-type | "unit"
```

Parameter and argument lists do not accept trailing commas. Alias parameter
syntax is parsed uniformly for functions, external declarations,
initializers, and methods; later semantic rules decide which declarations and
named types are legal. Alias targets retain their separate grammar and do not
accept `shared T`; primitive and named inline `T?` forms may designate
supported optional containers. Resolution rejects interface payloads for
inline optionals. `Obj` is
legal as the target of `shared Obj` and `shared? Obj`, but not as `Obj?`.
`unit` is syntactically restricted to result positions and `unit?` is rejected.
Nested optionals, optional references, `shared T?`, and `shared? T?` are
diagnosed with recovery rather than entering the AST.
Postfix `[]` binds inside a leading `shared` or `shared?`, so `shared T[]`
means a shared array owner. Grouping moves ownership into the element:
`(shared T)[]` is an inline array of shared owners. The rule composes
recursively. Type grouping is accepted only when followed by at least one
`[]`; it exists to preserve ownership grouping rather than as a general
redundant-parenthesis form. `?` may precede an array suffix to form optional
elements, but it may not follow an array suffix. `unit[]` is parsed so later
semantic analysis can report element ineligibility; bare `unit` remains
restricted to result positions.
Compilation-unit, namespace, entry-point, and external-signature semantics are
defined by [modules and foreign interoperation](MODULES_AND_INTEROP.md).

## Class declarations

```text
class-declaration           = "class" identifier ["extends" identifier]
                              ["implements" identifier {"," identifier}]
                              "{" {class-member} "}"

class-member                = field-declaration
                            | initializer-declaration
                            | copy-constructor-declaration
                            | copy-assignment-declaration
                            | destruction-declaration
                            | method-declaration

field-declaration           = identifier ":" storage-type ";"
initializer-declaration     = "init" parameter-list block
copy-constructor-declaration = "copy" parameter-list block
copy-assignment-declaration = "assign" parameter-list block
destruction-declaration     = "destroy" block
method-declaration          = [method-modifier] ["mut"] "fn" identifier parameter-list
                              "->" result-type block
method-modifier             = "virtual" | "override"

interface-declaration       = "interface" identifier
                              "{" {interface-requirement} "}"
interface-requirement       = ["mut"] "fn" identifier parameter-list
                              "->" result-type ";"
```

The grammar intentionally does not encode base-name resolution, hierarchy
validity, the required number or signature of lifecycle members,
initializer-body restrictions, receiver access, or member type legality. It
only classifies their source forms. A lifecycle word used after `fn` is an
ordinary method name; a lifecycle word followed by `:` is an ordinary field
name.

### Construction-selection syntax

The implemented `copy` declaration occupies a separate lifecycle slot. Its
required semantic shape is `copy(ref source: EnclosingClass) { ... }`; parsing
retains the general parameter-list shape so resolution can diagnose wrong
arity, binding mode, and target type precisely. An `init` declaration is
always ordinary, including `init(ref source: EnclosingClass)`.

Each class requires one or more `initializer-declaration` members, which form
an overload set. `Class(arguments)` uses the ordinary argument grammar and
selects the unique applicable, most-specific initializer from static argument
types. A derived initializer's leading `super(arguments)` applies the same
selection to the direct base's overload set. The copy constructor remains a
separate lifecycle slot and is never an ordinary initializer candidate.

Construction has two syntactically distinct argument modes:

```text
copy-construction-arguments  = "(" "copy" expression ")"
array-construction-arguments = "(" ")"
                             | "(" expression ")"
                             | "(" "copy" expression ")"
```

`Class(copy source)` uses `copy-construction-arguments`;
`Class(arguments)` never falls back to copy construction. The same distinction
applies to `new Class(copy source)` and `new Class(arguments)`.

The marker is contextual only immediately after the opening `(` and only when
followed by an expression. Consequently `Class(copy)` and
`Class(copy, other)` remain ordinary argument lists in which `copy` names a
binding. The explicit mode accepts exactly one source; a comma after that
source is an error.

`interface` and `implements` are contextual words. Interface bodies contain
signatures only: fields, lifecycle declarations, method bodies, inheritance,
and trailing separators are not part of this grammar. Name resolution
validates interface names in `implements` lists. Type checking validates exact
conformance, non-owning interface views, and interface calls into HIR. MIR and
its verifier represent those operations without choosing target table layouts;
the x86-64 backend owns and executes the resulting witness layout.

## Blocks and statements

```text
block                 = "{" {statement} "}"

statement             = local-declaration
                      | base-initialization
                      | return-statement
                      | conditional-statement
                      | assignment-statement
                      | expression-statement
                      | block

local-declaration     = "var" identifier ":" storage-type
                        "=" expression ";"
base-initialization   = "super" argument-list ";"
return-statement      = "return" [expression] ";"
expression-statement  = expression ";"

conditional-statement = "if" "(" expression ")" block
                        {"elif" "(" expression ")" block}
                        ["else" block]

assignment-statement  = place "=" expression ";"
place                 = place-atom {"." identifier}
place-atom            = identifier | "self" | "(" place ")"
```

The parser accepts an expression statement before semantic analysis decides
which expression results may be discarded. It likewise accepts a syntactic
place on the left of `=` before determining whether that place and value form
a legal primitive-field assignment, object assignment, or initialization.

An assignment whose ungrouped outer shape ends in `.member` is retained as a
field-assignment-shaped statement. Other place roots and explicitly grouped
complete places are retained as object-assignment-shaped statements. This is
source classification only; grouping does not determine the place's type or
mutability. An index or slice projection is likewise retained as an
assignment-shaped statement for later semantic classification.

`elif` is its own keyword and continuation form. `else if` and standalone
`elif` or `else` are not part of the grammar. Every conditional arm requires a
parenthesized expression and a block.

## Expressions

```text
expression       = additive-expression
                   ["is" (view-target | "some" | "none")]
view-target      = identifier

additive-expression
                 = multiplicative-expression
                   {("+" | "-") multiplicative-expression}

multiplicative-expression
                 = unary-expression {"*" unary-expression}

unary-expression = "-" unary-expression
                 | "*" unary-expression
                 | object-cast-expression
                 | postfix-expression

object-cast-expression
                 = "(" object-cast-target ")" unary-expression
object-cast-target
                 = view-target | "shared" view-target

postfix-expression
                 = primary-expression
                   {unwrap-suffix | member-suffix
                    | dereference-member-suffix | call-suffix
                    | index-or-slice-suffix | shared-index-or-slice-suffix}
unwrap-suffix    = "!"
member-suffix    = "." identifier
dereference-member-suffix
                 = "->" identifier
call-suffix      = "(" [argument-list] ")"
                 | copy-construction-arguments
index-or-slice-suffix
                 = "[" index-or-slice-bounds "]"
shared-index-or-slice-suffix
                 = "->" "[" index-or-slice-bounds "]"
index-or-slice-bounds
                 = expression
                 | [expression] ":" [expression]
argument-list    = expression {"," expression}

primary-expression
                 = "none"
                 | identifier
                 | literal
                 | "self"
                 | allocation-expression
                 | array-construction-expression
                 | "(" expression ")"

allocation-expression
                 = "new" identifier allocation-arguments
allocation-arguments
                 = "(" [argument-list] ")"
                 | copy-construction-arguments
array-construction-expression
                 = array-inline-type array-construction-arguments
                 | "new" array-inline-type array-construction-arguments
array-inline-type
                 = postfix-array-type
```

Leading outer `shared` or `shared?` belongs in a storage type, while `new`
selects shared construction.
The contextual `copy` form is a dedicated array construction mode and accepts
exactly one source. The ordinary nonempty form accepts exactly one length
expression.

From tightest to loosest binding, precedence is:

1. postfix unwrap, member access, dereferencing member access, calls, indexing,
   and slicing;
2. unary `-`, unary `*`, and object casts;
3. binary `*`;
4. binary `+` and `-`;
5. contextual `is`.

Postfix and binary operators associate left to right. Unary `-` and `*`
associate right to left. `is` is non-associative, so chained tests are syntax
errors. Grouping overrides precedence and remains represented in the
source-shaped syntax tree. `*owner.field` therefore means `*(owner.field)`;
use `(*owner).field` or `owner->field` to select a member from `owner`'s
pointee. Binary multiplication remains distinct by operator position, as in
`value * *owner`. Allocation and `none` are primary expressions. Calls,
postfix `!`, `.`, `->`, indexing, and slicing may participate in the same
postfix chain; type checking rejects chains that are not meaningful for the
operand type. `owner->[index]` and `owner->[start:end]` preserve a distinct
shared-projection operator from ordinary `owner[index]` and explicit
`(*owner)[index]`.
These spellings are semantically distinct: `.` remains within an already
selected inline place, while `->` crosses exactly one shared edge. There is no
implicit shared dereference.
Declaration selection and call legality are semantic concerns.

A parenthesized identifier followed by an adjacent expression is an object-cast
candidate. Cast syntax deliberately wins over grouped callable spelling:
`(f)(argument)` is resolved as a cast candidate, while direct calls use
`f(argument)`. Empty `()` is not an expression operand, and `(value) - other`
remains grouped subtraction. Postfix use of a cast requires grouping, as in
`((Leaf) value).read()`. `shared` is contextual in cast targets and
stored/result types. `new` is contextual only when followed by an identifier
and allocation argument list; `new()` remains an ordinary call to a binding
named `new`.

## Syntax errors and nesting

Unrecognized characters and malformed numeric-looking spellings produce
lexical diagnostics and scanning continues. The parser can report multiple
independent errors by recovering at parameter, argument, statement,
class-member, block, and top-level declaration boundaries. Invalid syntax is
never accepted merely because recovery reaches later source.

The current compiler limits simultaneously active recursive syntax constructs
to 128 levels. Class bodies, function and class-member bodies, nested blocks,
grouped expressions, unary expressions, nested calls, and postfix chains share
this budget. Recursive array type grouping and postfix array dimensions use
the same budget.
Exceeding it reports `PAR005`, omits the affected declaration from the partial
syntax tree, and resumes at a later top-level declaration when possible.

Exact synchronization mechanics and the partial tree retained after erroneous
input are compiler behavior, not additional grammar productions. Later
semantic phases run only when preceding lexical and syntax phases have no
errors.

## Semantic boundary

The grammar deliberately permits some forms that later analysis rejects. In
particular, it does not define:

- declaration lookup, namespaces, scope, or entry-point validity;
- type compatibility, literal ranges, operator availability, or return rules;
- whether an expression statement may discard its result;
- alias source, lifetime, access, or declaration-context restrictions;
- class member uniqueness, initializer completeness, containment, or receiver
  access;
- copy, assignment, destruction, temporary, or evaluation semantics;
- foreign-call legality, target representation, or runtime behavior.

Optional type and expression shapes cross lexing, parsing, and name resolution
with explicit nodes and flat resolved target identities. Primitive and
exact-class inline optionals cross explicit HIR, MIR, verification, x86-64
layout, and execution, including bounded checked class payload views. Optional
shared owners and aliases to supported inline optional containers cross the
same explicit phase boundary. The complete implemented semantics belong
to [Optional Values](OPTIONAL_VALUES.md).

Array tokens, recursive type grouping, construction modes, index and slice
shapes, and explicit shared bracket projection cross the syntax boundary with
deterministic AST dumps. Resolution assigns deterministic canonical recursive
array identities, and type checking retains structured lifecycle,
construction, projection, assignment, slice, and alias operations. These
forms lower to verified target-independent MIR. The x86-64 backend executes
inline and shared-outer arrays containing primitives, optionals, exact
classes, recursively nested inline arrays, and ordinary or optional shared
owners of exact classes and arrays. Construction, immutable length, checked
element access, named deep copy, produced-backing adoption, arbitrary-length
replacement, class fields, internal owning parameters/results, secure
shared-element replacement, deterministic cleanup, copied slices, and checked
equal-length slice assignment execute. Call-scoped whole-array and exact
element aliases execute with hidden backing or shared-owner anchors. The
implemented semantics belong to [Arrays](ARRAYS.md).

Use the [language overview](README.md) for the broad model and the
[status matrix](STATUS.md) for the implemented semantic boundary.
[Types, values, and expressions](TYPES_AND_VALUES.md) owns the detailed
semantics of literals, exact types, expression values, and operators.
[Functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md) owns callable,
scope, statement, return, and evaluation-order semantics.
[Classes and lifecycle](CLASSES_AND_LIFECYCLE.md) owns class declarations,
member rules, containment, receivers, initialization, and object places.
[Aliases and ownership](ALIASES_AND_OWNERSHIP.md) owns alias eligibility,
access, forwarding, overlap, and lifetime.
[Shared ownership and heap allocation](SHARED_OWNERSHIP.md) defines
`shared T`, ordinary `new T(arguments)`, and explicit copy-allocation
`new T(copy source)` semantics. The compiler implements these forms through
resolved identities, typed owner provenance, verified MIR lifetimes, native
allocation and deterministic last-owner destruction. The copy marker is not an
ordinary initializer argument.
[Object casts](OBJECT_CASTS.md) defines `(T) source` and `(shared T) source`
forms, precedence, and type-name disambiguation. Plain casts are currently
implemented for non-owning receiver, alias-argument, and field consumers plus
owning inline and shared copy construction, assignment, value arguments,
results, and slicing. Shared-owner casts execute without allocating or copying
payload.
[Polymorphism](POLYMORPHISM.md) owns inheritance, dispatch, interface views,
type tests, and checked-cast semantics.
