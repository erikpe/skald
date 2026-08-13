# Implemented Skald Grammar

Status: authoritative grammar for the source syntax accepted by the current
compiler. [Feature status](STATUS.md) determines whether the syntactic forms
below have a complete semantic implementation. Explicitly marked frozen
extensions record syntax selected for later implementation and are not
accepted by the current compiler.

This document defines tokens, concrete source shape, precedence,
associativity, and syntax-error boundaries. It does not define name lookup,
types, ownership, evaluation, lifecycle behavior, ABI, or lowering.

Module imports, visibility, and qualified declaration names below are part of
the implemented complete-compiler grammar. Their lookup and visibility
semantics are owned by
[Modules and Foreign Interoperation](MODULES_AND_INTEROP.md#import-syntax).
`import`, `from`, `as`, and `public` remain contextual identifiers.

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
while break continue
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
`private` is contextually recognized before an ordinary `init` declaration,
a field declaration, or the optional `mut` or `static` of a direct method
declaration. `static` is
contextually recognized before `fn` in a class member, including after
`private`. Both remain ordinary
identifier elsewhere, including in `private: i64;`, `fn private() -> unit {}`,
`static: i64;`, `fn static() -> unit {}`, parameters, locals, and top-level
declarations.
`implements`, `interface`, and `is` are likewise contextual in the exact forms
below. `where` is contextual after the optional base and interface clauses of
a class header that declares generic parameters; it remains an identifier in
all other positions. `some` is contextual only after `is`; `none` is reserved by the lexer
and forms either an absent expression or the target of a presence test.
`shared` is contextual before an object or array target in stored and result
types and inside a cast target. `new` is contextual before a class allocation
or an array construction. Both remain ordinary identifiers elsewhere.
`import`, `from`, and `as` are contextual in file-leading import declarations;
`public` is contextual before a top-level declaration. `intrinsic` is
contextual only before `fn` in a top-level intrinsic-function declaration.
All four spellings remain ordinary identifiers outside those positions.

## Punctuation

The complete punctuation and operator token set outside literal delimiters is:

```text
( ) { } [ ] , : :: ; . -> + - * / % = == != < <= > >= ? ! ~ & | ^ << >> && ||
```

Double quotes delimit one string-literal token. Single quotes delimit one
byte-literal token of type `u8`; Skald has no character type. There is no power
token in the implemented grammar. Power syntax is not frozen or reserved.

## Literals

```text
decimal-digits  = digit { digit }
digit           = "0".."9"
hex-prefix      = "0x" | "0X"
hex-digit       = digit | "a".."f" | "A".."F"
hex-digits      = hex-digit { hex-digit }

i64-literal     = decimal-digits | hex-prefix hex-digits
u64-literal     = decimal-digits "u" | hex-prefix hex-digits "u"
u8-literal      = decimal-digits "u8" | hex-prefix hex-digits "u8"

exponent        = ("e" | "E") ["+" | "-"] decimal-digits
f64-literal     = decimal-digits "." decimal-digits [exponent]
                | decimal-digits exponent

bool-literal    = "true" | "false"
numeric-literal = i64-literal | u64-literal | u8-literal | f64-literal
byte-literal    = single-quote (direct-byte | byte-escape) single-quote
direct-byte     = printable-ascii-except-single-quote-or-backslash
byte-escape     = backslash ( single-quote | double-quote | backslash
                              | "n" | "r" | "t" | "0"
                              | "x" hex-digit hex-digit )
string-literal  = double-quote { printable-ascii | string-escape } double-quote
string-escape   = backslash ( double-quote | backslash | "n" | "r" | "t"
                             | "0" | "x" hex-digit hex-digit )
literal         = numeric-literal | byte-literal | bool-literal | string-literal
```

Hexadecimal prefixes and digits are case-insensitive; integer suffixes remain
lowercase and case-sensitive. Leading `-` is a separate unary token. A decimal
point requires digits on both sides, and an exponent requires at least one
digit. Numeric-looking suffixes or tails not present above make the complete
numeric-looking spelling invalid; examples include `1_000`, `0x`, `0xg`,
`0xffu64`, `0x_ff`, `0x1.0`, `1.`, `1e+`, `1.2.3`, and `42u64`. A
leading-dot numeric-looking spelling such as `.5` is rejected as one malformed
token. Literal ranges and value interpretation are semantic rules, not grammar.

Unescaped string content is printable ASCII other than double quote and
backslash. The exact escapes, decoded bytes, and invalid-content behavior are
defined by [Skald Strings](STRINGS.md#string-literals). A physical newline,
direct non-ASCII content, unknown or incomplete escape, or missing closing
quote invalidates the complete literal token; parser recovery does not create
a string expression for it.

A byte literal decodes to exactly one byte. Direct content is one printable
ASCII byte other than single quote or backslash; the escapes are exactly those
in `byte-escape` above. Empty, multiple-byte, direct control-byte, direct
non-ASCII, unknown or incomplete escape, physical-newline, and unterminated
spellings are malformed. Recovery consumes through a closing quote when one is
available and otherwise stops before a physical newline. A byte literal is
always `u8`, not a Unicode scalar, code point, grapheme, or character value.

## Compilation unit and declarations

```text
compilation-unit              = { import-declaration }
                                { top-level-declaration } EOF

import-declaration            = module-import | selective-import
module-import                 = "import" module-path ["as" identifier] ";"
selective-import              = "from" module-path "import" imported-declaration
                                {"," imported-declaration} ";"
imported-declaration          = identifier ["as" identifier]
primitive-module-name         = "i64" | "u64" | "u8" | "f64" | "bool"
module-path-component         = identifier | primitive-module-name
module-path                   = module-path-component
                                {"::" module-path-component}
qualified-declaration-path    = module-path-component
                                {"::" module-path-component} "::" identifier
declaration-path              = identifier | qualified-declaration-path

top-level-declaration         = ["public"] (
                                  function-definition
                                | intrinsic-function-declaration
                                | external-function-declaration
                                | class-declaration
                                | interface-declaration
                                )

function-definition           = "fn" identifier parameter-list
                                "->" result-type block

external-function-declaration = "extern" "fn" identifier parameter-list
                                "->" result-type ";"

intrinsic-function-declaration = "intrinsic" "fn" identifier parameter-list
                                "->" result-type ";"

parameter-list                = "(" [parameter {"," parameter}] ")"
parameter                     = value-parameter | alias-parameter
value-parameter               = identifier ":" storage-type
alias-parameter               = ["mut"] "ref" identifier ":" storage-type

primitive-type                = "i64" | "u64" | "u8" | "f64" | "bool"
generic-argument-list         = "<" storage-type {"," storage-type} ">"
named-type                    = declaration-path [generic-argument-list]
type-primary                  = primitive-type | named-type | "unit"
                              | "(" storage-type ")"
postfix-type                  = type-primary {"?" | "[" "]"}
shared-type                   = "shared" ["?"] storage-type
storage-type                  = postfix-type | shared-type
result-type                   = storage-type | "unit"
```

Primitive type spellings remain reserved as declaration and binding names,
but may identify a module namespace inside a module or qualified declaration
path. This permits standard-library paths such as `std::f64` and qualified
uses such as `std::f64::to_bits`; it does not make `f64` an ordinary
identifier in any other context.

Imports must precede declarations. Selective import lists, parameter lists,
and argument lists do not accept trailing commas. Wildcard imports,
multi-segment aliases, relative imports, and empty module components are not
grammar forms. A `declaration-path` with more than one component is later
resolved through a direct module binding; an unqualified path uses the
module's ordinary namespace.

Alias parameter syntax is parsed uniformly for functions, external
declarations, initializers, and methods. Later semantic rules decide which
storage types may be designated; shared-owner and `unit` aliases remain
unavailable.

Postfix `?` and `[]` associate from left to right, and general type grouping
selects the complete operand to wrap. A leading `shared` consumes the complete
following storage type, including its postfix suffixes. Consequently `T?[]`
is an array of optional elements, `T[]?` is an optional inline array,
`shared T?` is a shared box whose target is optional, and `(shared T)?` is an
optional ordinary shared owner. `shared? T` is source shorthand for
`(shared T)?`; syntax retains which spelling was written while resolution
normalizes both through the same existing optional-owner semantics.

The parser admits nested optionals, optional arrays, and shared boxes so their
complete source shapes reach semantic analysis. Nested optionals and optional
inline arrays execute in every supported owning, aggregate, internal callable,
array-element, and checked-alias position. Semantic analysis still rejects
`unit?` and standalone owning optional interface or `Obj` values. Shared
optional box types and allocations receive canonical resolved identities.
Construction, stored and internal callable owners, arrays, and explicit exact
or polymorphic pointee access execute. Their source and compiler semantics are defined in
[Optional Values](OPTIONAL_VALUES.md#shared-optional-boxes). Optional
references such as `ref?` remain syntax errors.
`unit[]` is likewise parsed so later semantic analysis can report element
ineligibility; bare `unit` remains restricted to result positions.
Compilation-unit, namespace, entry-point, and external-signature semantics are
defined by [modules and foreign interoperation](MODULES_AND_INTEROP.md).

## Class declarations

```text
generic-parameter-list     = "<" identifier {"," identifier} ">"
generic-where-clause       = "where" generic-requirement
                             {"," generic-requirement}
generic-requirement        = identifier ":" declaration-path

class-declaration           = "class" identifier [generic-parameter-list]
                              ["extends" named-type]
                              ["implements" declaration-path {"," declaration-path}]
                              [generic-where-clause]
                              "{" {class-member} "}"

class-member                = field-declaration
                            | static-field-declaration
                            | initializer-declaration
                            | copy-constructor-declaration
                            | copy-assignment-declaration
                            | destruction-declaration
                            | method-declaration

field-declaration           = ["private"] identifier ":" storage-type ";"
static-field-declaration    = ["private"] "static" identifier ":"
                              storage-type ["=" expression] ";"
initializer-declaration     = ["private"] "init" parameter-list block
copy-constructor-declaration = "copy" parameter-list block
copy-assignment-declaration = "assign" parameter-list block
destruction-declaration     = "destroy" block
method-declaration          = public-method-declaration
                            | private-method-declaration
                            | static-method-declaration
public-method-declaration   = [method-modifier] ["mut"] "fn" identifier parameter-list
                              "->" result-type block
private-method-declaration  = "private" ["mut"] "fn" identifier parameter-list
                              "->" result-type block
static-method-declaration   = ["private"] "static" "fn" identifier parameter-list
                              "->" result-type block
method-modifier             = "virtual" | "override"

interface-declaration       = "interface" identifier
                              "{" {interface-requirement} "}"
interface-requirement       = ["mut"] "fn" identifier parameter-list
                              "->" result-type ";"
```

Generic parameter, argument, and requirement lists are nonempty and do not
accept trailing commas. Nested generic closers are interpreted in type context,
so `Outer<Inner<Str>>` is a named type while `left >> right` remains an
expression shift. Syntax preserves each angle bracket, comma, colon, grouping,
and `where` span independently.

Resolution assigns generic declarations stable non-executable
template and ordered parameter identities, includes templates in ordinary
module lookup and visibility, and diagnoses raw names, wrong declaration
kinds, and incorrect arity. Resolution also preserves structural
parameter-bearing types, fixes definition-site names, resolves nominal
interface bounds, classifies dependent body selections, infers contextual
requirements, and specializes requested closed declarations and bodies into
ordinary class and callable identities without introducing executable
placeholder types. Accepted closed applications use the ordinary lifecycle,
HIR, MIR, verification, and backend pipeline.

The grammar intentionally does not encode base-name resolution, hierarchy
validity, the required number or signature of lifecycle members,
initializer-body restrictions, receiver access, or member type legality. It
only classifies their source forms. A lifecycle word used after `fn` is an
ordinary method name; a lifecycle word followed by `:` is an ordinary field
name. Fields, methods, and ordinary initializers are public unless prefixed by
`private`. Copy constructors, copy assignments, and destructors do not accept
visibility, and no lifecycle declaration accepts `static`. Ordinary
initializers do not accept `mut`, `virtual`, or `override`; static methods
cannot use those modifiers either. Static fields accept an optional
declaration expression after `=`. `static` remains contextual: `static:` and
`private static:` continue to declare an instance field whose identifier is
`static`, while `static` followed by another identifier and `:` selects the
static-field form. Methods, functions, parameters, locals, and other
existing identifier positions retain their current use of the spelling.

Syntax and resolution retain static declarations, optional initializer
expressions, and their inherited identity. Static-field parsing also retains
`unit` so semantic rules can reject that otherwise well-formed type at the
declaration's type span. The grammar alone does not decide which storage types
are zero-valid or define executable access, initialization, mutation, or
lifetime. Those settled rules belong to [Static Fields](STATIC_FIELDS.md).
Type checking lowers all supported stored static types—including primitives,
exact classes, optionals, shared owners, strings, and inline arrays—through
typed HIR and verified lifecycle MIR.

### Construction-selection syntax

The implemented `copy` declaration occupies a separate lifecycle slot. Its
required semantic shape is `copy(ref source: EnclosingClass) { ... }`; parsing
retains the general parameter-list shape so resolution can diagnose wrong
arity, binding mode, and target type precisely. An `init` declaration is
always ordinary, including `init(ref source: EnclosingClass)`.

Each class requires one or more `initializer-declaration` members, which form
an overload set. Visibility belongs to each overload. `Class(arguments)` uses
the ordinary argument grammar and selects the unique applicable,
most-specific initializer from static argument types before enforcing that
initializer's declaring-class access. A derived initializer's leading
`super(arguments)` applies the same selection and access rules to the direct
base's overload set. The copy constructor remains a separate lifecycle slot
and is never an ordinary initializer candidate.

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
                      | while-statement
                      | break-statement
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
while-statement       = "while" "(" expression ")" block
break-statement       = "break" ";"

assignment-statement  = place "=" expression ";"
place                 = place-atom {"." identifier}
place-atom            = identifier | "self" | "(" place ")"
```

The parser accepts an expression statement before semantic analysis decides
which expression results may be discarded. It likewise accepts a syntactic
place on the left of `=` before determining which implemented assignment
category that source shape may denote.

An assignment whose ungrouped outer shape ends in `.member` is retained as a
field-assignment-shaped statement. Other place roots and explicitly grouped
complete places are retained as object-assignment-shaped statements. This is
source classification only; grouping does not determine the place's type or
mutability. An index or slice projection is likewise retained as an
assignment-shaped statement for later semantic classification.
Consequently, both `name = value;` and `(name) = value;` are accepted
assignment-shaped syntax without the parser deciding whether `name` denotes a
primitive binding, object, shared owner, optional, or array. Primitive binding
reassignment has
[implemented semantics](FUNCTIONS_AND_CONTROL_FLOW.md#primitive-binding-reassignment)
for exactly typed initialized primitive `var` locals and value parameters.

`elif` is its own keyword and continuation form. `else if` and standalone
`elif` or `else` are not part of the grammar. Every conditional arm requires a
parenthesized expression and a block.

### While loops and loop exits

The implemented loop and exit syntax is:

```text
while-statement    = "while" "(" expression ")" block
break-statement    = "break" ";"
continue-statement = "continue" ";"
```

The parentheses and body block are mandatory. `while` is not an expression,
loop exits carry no value, and labels are not part of the exit syntax.
`break` and `continue` must end with `;` and may appear only inside a loop.
The corresponding semantics are owned by
[Functions and Control Flow](FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits).

## Expressions

```text
expression                = logical-or-expression
view-target               = named-type

logical-or-expression     = logical-and-expression
                            {"||" logical-and-expression}
logical-and-expression    = comparison-expression
                            {"&&" comparison-expression}

comparison-expression     = bitwise-or-expression
                            [comparison-operator bitwise-or-expression]
                            ["is" (view-target | "some" | "none")]
comparison-operator
                          = "==" | "!=" | "<" | "<=" | ">" | ">="

bitwise-or-expression     = bitwise-xor-expression
                            {"|" bitwise-xor-expression}
bitwise-xor-expression    = bitwise-and-expression
                            {"^" bitwise-and-expression}
bitwise-and-expression    = shift-expression
                            {"&" shift-expression}

shift-expression          = additive-expression
                            {("<<" | ">>") additive-expression}

additive-expression
                          = multiplicative-expression
                            {("+" | "-") multiplicative-expression}

multiplicative-expression
                          = unary-expression
                            {("*" | "/" | "%") unary-expression}

unary-expression          = ("-" | "!" | "~" | "*") unary-expression
                          | cast-expression
                          | postfix-expression

cast-expression  = primitive-cast-expression
                 | object-cast-expression
primitive-cast-expression
                 = "(" primitive-type ")" unary-expression
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
                 | declaration-path
                 | generic-application
                 | generic-static-selection
                 | literal
                 | "self"
                 | allocation-expression
                 | array-construction-expression
                 | "(" expression ")"

allocation-expression
                 = class-allocation-expression
                 | optional-box-allocation-expression
class-allocation-expression
                 = "new" named-type allocation-arguments
optional-box-allocation-expression
                 = "new" storage-type "(" [expression] ")"
allocation-arguments
                 = "(" [argument-list] ")"
                 | copy-construction-arguments
array-construction-expression
                 = array-inline-type array-construction-initializer
                 | "new" array-inline-type array-construction-initializer
array-construction-initializer
                 = array-construction-arguments
                 | array-element-list
array-inline-type
                 = postfix-array-type
array-element-list = "{" [expression {"," expression}] "}"
generic-application
                 = declaration-path generic-argument-list
generic-static-selection
                 = generic-application "::" identifier
```

A `generic-application` in expression position is accepted only as a class
construction head immediately followed by a call suffix or as the target of
`generic-static-selection`. Generic applications are also accepted in object
casts, type tests, class allocations, optional-box and array construction
types, and every declaration position that consumes `storage-type`.

The `storage-type` in `optional-box-allocation-expression` must have an
optional outer semantic constructor after grouping is removed. This admits
targets such as `T?`, `T??`, `(T?)`, `T[]?`, and `(shared T)?`, while keeping
ordinary class and inline/shared array construction in their existing
productions. An optional-box initializer has zero or one expression and does
not accept a trailing comma or `copy` marker.

Leading outer `shared` or `shared?` belongs in a storage type, while `new`
selects shared construction.
The contextual `copy` form is a dedicated array construction mode and accepts
exactly one source. The ordinary nonempty parenthesized form accepts exactly
one length expression. An element list is a distinct accepted source mode;
category availability after type checking is tracked in the
[status matrix](STATUS.md#not-implemented).

### Explicit array element lists

The explicit array type is required. Untyped
`[expression {"," expression}]`, expected-type-only lists, and multiple
ordinary expressions inside the existing parentheses are not accepted forms.

An empty list is valid, one or more elements are comma-separated, and the
grammar does not accept a trailing comma. The braces and every comma
remain exact source spans for syntax, recovery, and deterministic dumps.
Ordinary postfix operations may follow the closing brace. Whitespace does not
change the construction shape.

The parser and resolver retain the complete ordered list and exact array
identity. Type checking records one destination-directed initialization plan
per element, and verified MIR executes the ordered allocation, initialization,
publication, ownership, and cleanup protocol. The complete semantics are
defined by the
[array element-list contract](ARRAYS.md#explicit-element-list-construction).

From tightest to loosest binding, precedence is:

1. postfix unwrap, member access, dereferencing member access, calls, indexing,
   and slicing;
2. prefix `-`, `!`, `~`, and `*`, and primitive or object casts;
3. binary `*`;
4. binary `+` and `-`;
5. `&`;
6. `^`;
7. `|`;
8. primitive comparisons `==`, `!=`, `<`, `<=`, `>`, and `>=`;
9. contextual `is`;
10. `&&`;
11. `||`.

Postfix, arithmetic, shift, bitwise, and logical binary operators associate left to
right. Prefix `-`, `!`, `~`, and `*` associate right to left. Comparisons and
`is` are non-associative, so ungrouped chained comparisons or tests are syntax
errors. Grouping overrides precedence and remains represented in the
source-shaped syntax tree.
`*owner.field` therefore means `*(owner.field)`; use `(*owner).field` or
`owner->field` to select a member from `owner`'s pointee. Binary multiplication
remains distinct by operator position, as in `value * *owner`. Allocation and
`none` are primary expressions. Calls, postfix `!`, `.`, `->`, indexing, and
slicing may participate in the same postfix chain; type checking rejects
chains that are not meaningful for the operand type. `owner->[index]` and
`owner->[start:end]` preserve a distinct shared-projection operator from
ordinary `owner[index]` and explicit `(*owner)[index]`.
These spellings are semantically distinct: `.` remains within an already
selected inline place, while `->` crosses exactly one shared edge. There is no
implicit shared dereference.
Declaration selection and call legality are semantic concerns.

Postfix unwrap binds above prefix logical negation and bitwise complement, so
`!optional_flag!` means `!(optional_flag!)`, `~optional_byte!` means
`~(optional_byte!)`, and `!!flag` means `!(!flag)`. Operator position keeps the
uses distinct, while longest-match tokenization keeps `!=`, `&&`, and `||` as
single tokens.

A primitive keyword in this position unambiguously selects a primitive cast.
A parenthesized identifier followed by an adjacent expression is an
object-cast candidate. Cast syntax deliberately wins over grouped callable
spelling: `(f)(argument)` is resolved as a cast candidate, while direct calls
use `f(argument)`. Empty `()` is not an expression operand, and
`(value) - other` remains grouped subtraction. Postfix use of a cast requires
grouping, as in `((Leaf) value).read()`. `shared` is contextual in cast targets
and stored/result types. `new` is contextual only when followed by an
identifier and allocation argument list; `new()` remains an ordinary call to
a binding named `new`.

Primitive and object casts retain the existing unary precedence and
right-associative operand shape. Each primitive keyword (`i64`, `u64`, `u8`,
`f64`, or `bool`) unambiguously selects a primitive cast target, while a
declaration path or `shared` declaration path selects the existing object-cast
syntax. Postfix use of either cast still requires grouping. Syntax and
resolution preserve all five primitive targets. Type checking accepts all
twenty-five source/target pairs; the three `f64`-to-integer pairs are checked
operations that may terminate at runtime. The integer subset and complete
primitive cast matrix are defined by
[Types, Values, and Expressions](TYPES_AND_VALUES.md#explicit-integer-casts)
and its
[complete matrix](TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix).

### Implemented primitive-operator expressions

The complete primitive-operator grammar has the following implemented shape:

```text
expression                = logical-or-expression

logical-or-expression     = logical-and-expression
                            {"||" logical-and-expression}

logical-and-expression    = comparison-expression
                            {"&&" comparison-expression}

comparison-expression     = bitwise-or-expression
                            [comparison-suffix]
comparison-suffix         = comparison-operator bitwise-or-expression
                          | "is" (view-target | "some" | "none")
comparison-operator       = "==" | "!=" | "<" | "<=" | ">" | ">="

bitwise-or-expression     = bitwise-xor-expression
                            {"|" bitwise-xor-expression}
bitwise-xor-expression    = bitwise-and-expression
                            {"^" bitwise-and-expression}
bitwise-and-expression    = shift-expression
                            {"&" shift-expression}

shift-expression          = additive-expression
                            {("<<" | ">>") additive-expression}
additive-expression       = multiplicative-expression
                            {("+" | "-") multiplicative-expression}
multiplicative-expression = unary-expression
                            {("*" | "/" | "%") unary-expression}

unary-expression          = ("-" | "!" | "~" | "*") unary-expression
                          | cast-expression
                          | postfix-expression
```

The cast and postfix productions retain their implemented definitions. From
tightest to loosest binding, the precedence is:

1. postfix unwrap, member access, dereferencing member access, calls, indexing,
   and slicing;
2. prefix `-`, `!`, `~`, explicit shared dereference `*`, and primitive or
   object casts;
3. binary `*`, `/`, and `%`;
4. binary `+` and `-`;
5. `<<` and `>>`;
6. `&`;
7. `^`;
8. `|`;
9. `==`, `!=`, `<`, `<=`, `>`, `>=`, and contextual `is`;
10. `&&`;
11. `||`.

Postfix, arithmetic, shift, bitwise, and logical chains associate left to
right. Prefix operators associate right to left. The complete comparison tier
is non-associative: it accepts at most one comparison or `is` suffix without
grouping. Contextual `is` retains its specialized type or presence-test right
side; it is not identity equality and there is no `is not` syntax.

Postfix `!` binds above prefix `!`, so these shapes are unambiguous:

```ska
!value!      // !(value!)
!!flag       // !(!flag)
value!!      // (value!)!; checks two nested optional layers
left != right
```

Whitespace does not change those parses. Longest-match tokenization keeps
`!=`, `<<`, `>>`, `&&`, and `||` intact. Forms such as
`first < second < third` and `value is Item == flag` are invalid comparison
chains; grouping may turn an intermediate result into an ordinary `bool`
operand.

Existing `//` line-comment recognition remains distinct from division: adjacent
`//` begins a comment, while `/ /` is two division tokens separated by trivia.

The source semantics and exact operand matrix are defined by the
[implemented primitive operator profile](TYPES_AND_VALUES.md#implemented-primitive-operator-profile).
The [status matrix](STATUS.md) remains authoritative for compiler support.

## Syntax errors and nesting

Unrecognized characters and malformed numeric-looking spellings produce
lexical diagnostics and scanning continues. The parser can report multiple
independent errors by recovering at parameter, argument, statement,
class-member, block, and top-level declaration boundaries. Invalid syntax is
never accepted merely because recovery reaches later source.

The current compiler limits simultaneously active recursive syntax constructs
to 128 levels. Class bodies, function and class-member bodies, nested blocks,
grouped expressions, unary expressions, primitive and object casts, nested
calls, and postfix chains share this budget. Recursive array type grouping and
postfix array dimensions use the same budget.

Separately, one expression-tree path may contain at most 10 nested
short-circuit operations. A flat left-associated chain can therefore contain
up to 11 operands. Logical chains are parsed iteratively, but their selected
path conditions remain live to a shared full-expression boundary. Equivalent
verifier states are compacted, but effectful alternatives can carry genuinely
different ownership and cleanup state; the smaller logical limit bounds that
path-sensitive work and conditional-cleanup graph construction.

Exceeding either limit reports `PAR005`, omits the affected declaration from
the partial syntax tree, and resumes at a later top-level declaration when
possible.

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

The existing `argument-list` grammar already admits an exact-class-producing
expression wherever a call argument is written. The
[implemented produced read-only alias contract](ALIASES_AND_OWNERSHIP.md#implemented-produced-read-only-alias-arguments)
therefore adds no token, precedence level, expression node, reference
expression, or call form. Resolution continues to retain the ordinary source
expression. Alias type checking and HIR admit direct compatible
exact-class producers only for read-only `ref` parameters, while `mut ref`
remains place-based. Verified MIR lifetime lowering and native execution
require no grammar change.

The same postfix grammar already parses member selection and a call suffix on
an exact-class construction, class literal, or exact-class call result. The
frozen
[produced exact-class method-receiver contract](FUNCTIONS_AND_CONTROL_FLOW.md#frozen-produced-exact-class-method-receivers)
therefore adds no token, precedence rule, AST expression shape, or call
syntax. Current semantic analysis still rejects those producers as method
receivers; implementation changes only their resolved and typed eligibility.

Optional type syntax crosses parsing as a recursive source-shaped node that
retains grouping, punctuation, and `shared?` shorthand provenance. Resolution
interns deterministic recursive optional identities for eligible primitive,
exact-class, shared-owner, optional, and inline-array payloads. These types
cross explicit HIR, MIR, verification, x86-64 layout, and execution, including
bounded checked payload views and aliases to supported inline optional
containers. The complete implemented semantics belong to
[Optional Values](OPTIONAL_VALUES.md).

The type grammar preserves the implemented shared-optional-box forms:
`shared P?` means `Shared<Optional<P>>`, `(shared P?)?` means an optional owner
of that non-null box, and `shared? P?` is exact shorthand for the latter. The
allocation grammar accepts `new P?()` and `new P?(expression)` with one
complete optional type target. The AST keeps this form distinct from class and
array construction and retains its type grouping and punctuation. Resolution
assigns its exact optional target and static box-view identity. Box
construction, owner lifetimes, immutable access, stored positions, and arrays
reach verified MIR and native x86-64 execution.

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

The implemented [standard I/O API](IO.md) composes existing modules, functions,
calls, loops, arrays, aliases, and intrinsic declarations. It adds no token,
precedence level, expression shape, statement shape, or declaration form.
This grammar therefore already describes its source shapes. The `std::io`
module installs its private canonical intrinsic declarations and implements
all nine public functions using existing language forms.

The implemented [process-argument API](PROCESS.md) likewise composes existing
declarations, calls, arrays, loops, strings, modules, and I/O. Its
`std::process` module adds no syntax or parser work.

The implemented
[primitive string conversion API](STRINGS.md#frozen-primitive-textual-conversions)
likewise composes existing static and instance methods, primitive parameters,
class results, and optional primitive results. Its accepted text is data
examined by ordinary library code, not additional Skald token or literal
syntax.

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
