# Implemented Skald Grammar

Status: authoritative grammar for the source syntax accepted by the current
compiler. [Feature status](STATUS.md) determines whether the syntactic forms
below have a complete semantic implementation.

This document defines tokens, concrete source shape, precedence,
associativity, and syntax-error boundaries. It does not define name lookup,
types, ownership, evaluation, lifecycle behavior, ABI, or lowering.

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
```

`init`, `assign`, and `destroy` are ordinary identifiers except in their direct
class-member forms:

- `init` followed by a parameter list begins an initializer;
- `assign` followed by a parameter list begins copy assignment;
- `destroy` followed directly by a block begins destruction.

They remain available as field names, method names, parameter names, local
names, and top-level function names. For example, `destroy: i64;` is a field
and `fn destroy() -> unit {}` is a method.

The frozen constructor extension additionally makes `copy` contextual in a
direct class-member declaration and immediately after the opening parenthesis
of a class construction. The current parser does not yet accept either shape;
`copy` remains an ordinary identifier everywhere in the implemented grammar.

`extends` is contextually recognized only after a class name. `super` followed
by a call argument list is contextually recognized as a dedicated statement;
resolution restricts it to the first statement of a derived ordinary
initializer. Both spellings remain ordinary identifiers outside those shapes.
`Obj` is contextually recognized as the universal object-view type in
alias-parameter and type-operation target positions; it remains an ordinary
identifier elsewhere except that it cannot name a top-level declaration.
`virtual` and `override` are contextually recognized only as method modifiers.
`implements`, `interface`, and `is` are likewise contextual in the
exact forms below. None is reserved by the lexer.

## Punctuation

The complete punctuation and operator token set is:

```text
( ) { } , : ; . -> + - * =
```

There are no string, character, comparison, division, bracket, or question-mark
tokens in the implemented grammar.

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
alias-target                  = identifier | "Obj"

primitive-type                = "i64" | "u64" | "u8" | "f64" | "bool"
named-type                    = identifier
storage-type                  = primitive-type | named-type
result-type                   = storage-type | "unit"
```

Parameter and argument lists do not accept trailing commas. Alias parameter
syntax is parsed uniformly for functions, external declarations,
initializers, and methods; later semantic rules decide which declarations and
named types are legal. `Obj` is legal only for non-owning internal alias
parameters. `unit` is syntactically restricted to result positions.
Compilation-unit, namespace, entry-point, and external-signature semantics are
defined by [modules and foreign interoperation](MODULES_AND_INTEROP.md).

## Class declarations

```text
class-declaration           = "class" identifier ["extends" identifier]
                              ["implements" identifier {"," identifier}]
                              "{" {class-member} "}"

class-member                = field-declaration
                            | initializer-declaration
                            | copy-assignment-declaration
                            | destruction-declaration
                            | method-declaration

field-declaration           = identifier ":" storage-type ";"
initializer-declaration     = "init" parameter-list block
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

### Frozen constructor syntax transition

The planned constructor model extends the class grammar with:

```text
copy-constructor-declaration = "copy" "(" "ref" identifier
                               ":" named-type ")" block
copy-construction-arguments  = "(" "copy" expression ")"
```

Each class will require one or more `initializer-declaration` members, which
form an overload set. A `copy-constructor-declaration` occupies one separate
lifecycle slot. `Class(copy source)` uses
`copy-construction-arguments`; `Class(arguments)` retains the ordinary call
argument grammar and never falls back to copy construction. The same
distinction will apply to future `new Class(copy source)` and
`new Class(arguments)`.

These productions are a frozen future source contract, not syntax accepted by
the current compiler. Until the prerequisite constructor roadmap lands, the
compiler accepts one ordinary `init` and semantically classifies the
single-`ref` exact-class `init` shape as its legacy copy constructor.

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
mutability.

`elif` is its own keyword and continuation form. `else if` and standalone
`elif` or `else` are not part of the grammar. Every conditional arm requires a
parenthesized expression and a block.

## Expressions

```text
expression       = additive-expression ["is" view-target]
view-target      = identifier

additive-expression
                 = multiplicative-expression
                   {("+" | "-") multiplicative-expression}

multiplicative-expression
                 = unary-expression {"*" unary-expression}

unary-expression = "-" unary-expression
                 | object-cast-expression
                 | postfix-expression

object-cast-expression
                 = "(" object-cast-target ")" unary-expression
object-cast-target
                 = view-target | "shared" view-target

postfix-expression
                 = primary-expression {member-suffix | call-suffix}
member-suffix    = "." identifier
call-suffix      = "(" [argument-list] ")"
argument-list    = expression {"," expression}

primary-expression
                 = identifier
                 | literal
                 | "self"
                 | "(" expression ")"
```

From tightest to loosest binding, precedence is:

1. postfix member access and calls;
2. unary `-` and object casts;
3. binary `*`;
4. binary `+` and `-`;
5. contextual `is`.

Postfix and binary operators associate left to right. Unary `-` associates
right to left. `is` is non-associative, so chained tests are syntax errors.
Grouping overrides precedence and remains represented in the source-shaped
syntax tree. Calls and member access may be interleaved in one postfix chain;
declaration selection and call legality are semantic concerns.

A parenthesized identifier followed by an adjacent expression is an object-cast
candidate. Cast syntax deliberately wins over grouped callable spelling:
`(f)(argument)` is resolved as a cast candidate, while direct calls use
`f(argument)`. Empty `()` is not an expression operand, and `(value) - other`
remains grouped subtraction. Postfix use of a cast requires grouping, as in
`((Leaf) value).read()`. `shared` is contextual only in the cast-target
position.

## Syntax errors and nesting

Unrecognized characters and malformed numeric-looking spellings produce
lexical diagnostics and scanning continues. The parser can report multiple
independent errors by recovering at parameter, argument, statement,
class-member, block, and top-level declaration boundaries. Invalid syntax is
never accepted merely because recovery reaches later source.

The current compiler limits simultaneously active recursive syntax constructs
to 128 levels. Class bodies, function and class-member bodies, nested blocks,
grouped expressions, unary expressions, and nested calls share this budget.
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
[Shared ownership and heap allocation](SHARED_OWNERSHIP.md) freezes future
`shared T`, ordinary `new T(arguments)`, and explicit copy-allocation
`new T(copy source)` source forms and semantics. The copy marker selects the
named class's copy constructor and target-directed checked source; it is not
an ordinary initializer argument. These forms remain outside this implemented
grammar until compiler support is added.
[Object casts](OBJECT_CASTS.md) defines `(T) source` and `(shared T) source`
forms, precedence, and type-name disambiguation. Plain casts are currently
implemented for non-owning receiver, alias-argument, and field consumers plus
owning inline copy construction, assignment, value arguments, results, and
slicing. `shared T` is parsed for a focused unsupported-feature diagnostic.
[Polymorphism](POLYMORPHISM.md) owns inheritance, dispatch, interface views,
type tests, and checked-cast semantics.
