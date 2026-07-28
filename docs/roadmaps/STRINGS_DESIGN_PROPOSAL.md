# String Types Design Proposal

Status: proposed design complete; freezing is blocked only on implemented
private fields, private methods, and static methods.

This proposal defines the intended first string model for Skald. It combines
the ordinary standard-library-class direction established during the Niflheim
audit with the small shared descriptor and static literal backing described by
the early Niflheim2 exploration. It is deliberately more detailed than an
implementation sketch so that, once its three member-feature prerequisites
exist, it can be checked against their implemented contracts and promoted
without reopening the string representation.

Until that promotion, the
[status matrix](../language/STATUS.md#not-implemented) remains authoritative:
strings are not accepted source syntax or an executable language contract.

## Intended outcome

The first Skald string type has these defining properties:

- every string is a finite sequence of `u8` bytes;
- the language assigns no Unicode, UTF-8, character, locale, collation, or
  normalization meaning to those bytes;
- the language-facing type is the ordinary public class declaration
  `std::str::Str`;
- a `Str` value is a small inline descriptor containing one shared byte-array
  owner and a byte range;
- copying or slicing a `Str` never copies its bytes;
- string literal bytes reside in compiler-emitted immutable program-lifetime
  storage and cause no allocation or byte copy when evaluated;
- dynamically produced strings use ordinary shared-array allocation and
  compiler-generated ownership operations;
- string operations are implemented in Skald standard-library source except
  for literal recognition and materialization; and
- no string-specific C runtime entry point or public native ABI is introduced.

The proposal treats `Str` as logically immutable. An existing string's
observable byte sequence never changes, while an owning variable containing a
`Str` remains assignable to another complete `Str` value.

## Prerequisites and freeze gate

The proposal depends on these general class-member features:

1. **Private fields.** A field marked `private` is selectable only from within
   the body of its declaring class. It is not exposed to another class in the
   same module, to derived classes, or to importers.
2. **Private methods.** An ordinary instance method marked `private` has the
   same declaring-class access boundary and is statically selected rather than
   participating in virtual dispatch.
3. **Static methods.** A static method belongs to one class identity, has no
   `self` receiver, is selected through that class, obeys ordinary visibility,
   parameter, result, evaluation-order, and ownership rules, and can access
   private members under the declaring-class rule.

Their exact accepted modifier order and diagnostics belong to their own
language and grammar contracts. The string proposal relies on their semantics,
not on provisional parser spellings.

Once all three features are implemented, this proposal should receive one
focused confirmation pass:

- update illustrative declarations to the implemented modifier grammar;
- confirm that language-item validation and intrinsic literal construction do
  not create a source-visible privacy escape;
- confirm that ordinary static factories and private helpers can express the
  dynamic construction paths described below; and
- promote the source and compiler rules into focused frozen string contracts.

No intentional string-design question remains after that pass. `final` fields,
static fields, loops, additional operators, and a complete standard-library
API are explicitly not freeze prerequisites.

## Canonical language item

The string language item is the exact public class declaration named `Str` in
the exact logical module `std::str`. Its canonical declaration path is:

```text
std::str::Str
```

The path is case-sensitive. A local class named `Str`, an imported class with
that leaf name, or another globally unique declaration named `Str` has no
special meaning.

Under the current module-provider mapping, a standard-library root contains
this module at:

```text
<stdlib-root>/std/str.ska
```

The physical layout may change only if the general standard-library provider
contract changes; string resolution continues to use the logical identity.

The responsible semantic phase resolves the language-item path once to a
`ClassId`. HIR, MIR, verification, and target lowering carry that identity and
must not repeat source-name or path-string comparisons.

The selected declaration must satisfy this structural contract:

- it is a public concrete class;
- it has no direct base class;
- its first three direct fields, in order, are exactly the private fields
  `storage: shared u8[]`, `start: u64`, and `length: u64`;
- it has no additional direct fields;
- it declares no explicit copy constructor, copy assignment, or destructor;
  and
- it therefore receives the ordinary synthesized field-wise lifecycle.

Interface conformance and ordinary or static methods do not alter the
language-item representation and remain permitted. A class derived from
`std::str::Str` is an ordinary distinct class; only the exact `Str` identity
is a string type or the type of a literal.

The compiler does not require, select, or lower through any method or
initializer name. In particular, it does not search for `from_bytes`,
`concat`, or an initializer by spelling. A conforming replacement standard
library may change its ordinary API without changing literal lowering.

## Conceptual class shape

The modifier ordering below is illustrative until the prerequisites freeze
their grammar:

```ska
public class Str {
    private storage: shared u8[];
    private start: u64;
    private length: u64;

    // Ordinary public instance methods.
    // Ordinary public static factories.
    // Private instance and static implementation helpers.
}
```

The fields mean:

| Field | Meaning |
|---|---|
| `storage` | One non-null shared owner of an exact `u8[]` allocation. |
| `start` | The first byte in `storage` belonging to this string. |
| `length` | The number of bytes belonging to this string. |

Every valid value satisfies:

```text
start <= storage.len()
length <= storage.len() - start
```

Compiler-created values establish this invariant mechanically. The standard
library is trusted to preserve it for values produced by ordinary Skald
methods and initializers. Public code cannot inspect or replace the descriptor
fields directly.

The source-visible contract does not freeze physical class size, alignment, or
field offsets. On the current x86-64 target the expected realization is three
eight-byte words, but ordinary target layout remains authoritative.

## Byte and literal semantics

### Source spelling

A string literal is delimited by double quotes and has the exact static type
identified by `std::str::Str`.

Unescaped literal content is restricted to printable ASCII bytes other than
`"` and `\`. This avoids making UTF-8 source encoding part of string value
semantics. All other byte values use escapes.

The initial escapes are:

| Escape | Produced byte |
|---|---:|
| `\"` | `0x22` |
| `\\` | `0x5c` |
| `\n` | `0x0a` |
| `\r` | `0x0d` |
| `\t` | `0x09` |
| `\0` | `0x00` |
| `\xNN` | the byte denoted by exactly two hexadecimal digits |

Hexadecimal digits are case-insensitive. Unknown escapes, incomplete
hexadecimal escapes, direct non-ASCII content, unescaped newlines, and an
unterminated literal are syntax diagnostics with source spans. Recovery must
not manufacture a valid string expression from invalid content.

Examples:

```ska
"hello"
"line one\nline two"
"\x00\x7f\x80\xff"
""
```

Literal length is the decoded byte count. A zero byte is ordinary content;
strings have no required terminator and no implicit extra byte. Equal decoded
byte sequences are equal string contents regardless of how their literals
were escaped.

### Evaluation

The compiler decodes and validates each literal before typed HIR. A literal is
a produced exact `Str` value with:

- `storage` set to a compiler-emitted immortal shared `u8[]` allocation
  containing the decoded bytes;
- `start` set to zero; and
- `length` set to the decoded byte count.

The descriptor follows ordinary inline-class destination and temporary rules.
It may be constructed directly in an eligible final destination. Named `Str`
copy construction copies the shared owner and scalar bounds; assignment uses
ordinary secure shared-owner replacement plus scalar assignment; destruction
releases the shared owner. For immortal literal storage those generated
retains and releases are no-ops.

Literal evaluation must not:

- allocate dynamic storage;
- copy the literal bytes;
- call a `Str` initializer, static method, or other standard-library code;
- introduce a distinct native string runtime operation; or
- expose the backing owner as a source value.

The compiler may pool identical decoded byte sequences and should emit one
canonical empty backing per program. Pooling order and symbols must be
deterministic. Backing identity is unobservable through the valid public
interface, so the language does not promise whether distinct literal
occurrences share one emitted allocation.

## Literal dependency and module behavior

A source module containing a valid string literal acquires a compiler-owned
language-item dependency on logical module `std::str`. Source code does not
need an explicit import merely to evaluate the literal.

This dependency participates in ordinary module discovery:

- the module is loaded at most once under its canonical identity;
- missing, ambiguous, unreadable, malformed, cyclic, private, wrong-kind, and
  structurally invalid language items are diagnosed through the responsible
  module or semantic boundary;
- provider order does not select between ambiguous `std::str` modules;
- `--no-stdlib` is valid when another configured provider supplies exactly one
  conforming `std::str` module; and
- a program using no string literal does not acquire this dependency merely
  because the compiler knows the language-item path.

The synthetic dependency grants only the compiler access required to validate
and materialize the exact language item. It does not introduce an unqualified
source binding, re-export the declaration, or permit ordinary source access to
private fields. Source code that explicitly names the class or its static
methods still follows normal direct-import and visibility rules.

The source-text convenience API currently has no module-provider context. It
must either be extended with an explicit provider-aware request or reject a
string literal with the ordinary missing-language-item diagnostic. It must not
silently synthesize a second built-in `Str` class.

## Shared immortality

### General representation state

Immortality is a compiler-private state of shared storage, not a string-only
array kind and not a source-visible ownership qualifier. It is available to
all exact shared allocation layouts whose first header word is the ordinary
strong count, including shared objects and shared arrays.

The strong-count value:

```text
IMMORTAL = u64::MAX
```

is reserved for a valid program-lifetime allocation. Generated ownership
operations behave as follows:

- retain of `IMMORTAL` succeeds without storing;
- release of `IMMORTAL` succeeds without storing, finalizing, or freeing;
- an ordinary positive dynamic count retains and releases under the existing
  rules;
- retaining an ordinary count of `u64::MAX - 1` terminates unsuccessfully
  rather than producing the reserved value; and
- zero remains invalid for an ordinary non-optional shared handle and reserved
  as the optional-owner absence niche where already specified.

Only a verified compiler static-allocation producer may initially publish an
immortal handle. Ordinary source `new`, dynamic array construction, count
operations, and the C runtime cannot manufacture the tag. MIR and backend
legality must distinguish a compiler-emitted immortal allocation from an
unpublished dynamic allocation rather than overloading ordinary count-one
publication.

This state may later support canonical empty allocations, interned immutable
data, or source-visible program-lifetime constants. It does not itself define
globals or static fields. A static variable may contain a normal mortal shared
owner, and a mutable static handle may be rebound; those features require
separate initialization, destruction, and concurrency semantics.

### Literal backing

On x86-64, a string literal backing uses the existing exact shared-array
layout:

| Offset | Width | Meaning |
|---:|---:|---|
| 0 | 8 | `IMMORTAL` |
| 8 | 8 | exact `u8[]` metadata/finalizer table pointer |
| 16 | 8 | decoded byte length |
| 24 | length | decoded bytes |

Required target padding follows ordinary exact-array layout. The allocation is
emitted in immutable or relocation-read-only program data. Its metadata is
valid even though the finalizer can never be selected by a correct release.

Other targets may choose another representation while preserving the shared
handle, array, and immortality invariants. The portable language contract does
not expose the sentinel, header, metadata, section, or symbol.

## Immutability boundary

Immortality controls lifetime; it does not make `shared u8[]` a read-only
source type. Likewise, a future `final storage` field would prevent replacing
the owner but would not prevent element mutation through that owner.

The first string profile establishes logical immutability through:

- private descriptor fields;
- no public method that exposes the backing owner or a mutable byte alias;
- no public mutable-receiver method that changes an existing string's
  observable range or bytes;
- physically immutable compiler-emitted literal backing;
- fresh unaliased backing for dynamically frozen content; and
- a trusted standard-library implementation that does not mutate storage
  reachable from a live `Str`.

A public constructor or static factory accepting caller-owned bytes must copy
those bytes into fresh shared storage. It cannot retain a caller-supplied
shared `u8[]`, because another owner could mutate the supposedly immutable
string. A mutable builder such as a future `StrBuf` similarly freezes by
copying into a fresh shared `u8[]` before returning `Str`.

An implementation-private helper inside `Str` may share an existing trusted
backing when creating a slice. Such a helper must preserve the range invariant
and must never receive arbitrary aliased mutable storage from public code.

This is an intentional trust boundary, not a claim of type-system-enforced
deep immutability. A later frozen or read-only shared type may strengthen the
boundary without changing the `Str` value or literal semantics.

## Value and operation costs

For valid `Str` values, the intended complexity is:

| Operation | Required behavior |
|---|---|
| Copy construction | `O(1)` descriptor copy and one shared retain |
| Copy assignment | `O(1)` secure incoming owner, release old owner, copy bounds |
| Destruction | `O(1)` shared release, with possible backing reclamation |
| Length | `O(1)` descriptor read |
| Byte access | `O(1)` checked range access |
| Slice | `O(1)` owner copy plus adjusted bounds |
| Convert from caller-owned bytes | `O(n)` fresh allocation and byte copy |
| Convert to independent `u8[]` | `O(n)` byte copy |
| Concatenation | `O(n + m)` fresh allocation and byte copies |

The last release of a dynamic backing frees it through the existing generic
shared-array finalization and allocation boundary. Literal backing never
reaches last-owner finalization because it is immortal.

The standard library is expected to provide ordinary instance operations and
static factories for these behaviors. Static construction helpers are a
prerequisite so factories remain class-namespaced without giving the compiler
special method spellings. Exact method names and the broader formatting,
parsing, searching, splitting, joining, and builder API belong to standard
library design and are not compiler contracts.

String indexing, slicing, comparison, equality, hashing, concatenation,
formatting, and parsing are not intrinsic operators in this proposal. They use
ordinary methods or static methods until a separate language feature defines
operator behavior. In particular, the compiler does not lower `+` by looking
for a method named `concat`.

## Final fields and static fields

Field-level `final` is deliberately not used by the language-item contract.
Under Skald's inline lifecycle model, a coherent final field may be initialized
during construction but cannot be replaced by copy assignment. Making all
three descriptor fields final would therefore make ordinary reassignment of a
`Str` unavailable unless the language separately defines whole-value
destruction and reconstruction. Private non-final fields plus a non-mutating
public interface provide the required first string semantics.

Static fields are also outside this proposal. Compiler-emitted literal data is
backend static storage, not a source-visible field. Source-visible static
state requires independent decisions about initialization order, cycles,
program-exit destruction, mutable access, ownership accounting, and eventual
concurrency. Some future immutable static values may use immortal backing, but
immortality does not imply that every static shared pointee is immortal.

## Compiler phase responsibilities

### Lexing and parsing

- Recognize double-quoted literals and their complete source spans.
- Decode only the specified ASCII and byte escapes.
- Diagnose malformed content deterministically and recover at an appropriate
  expression or declaration boundary.
- Preserve decoded bytes or a lossless source form; do not defer invalid byte
  conversion to the backend.

### Module discovery and resolution

- Add the language-item dependency only for modules containing valid string
  literal syntax.
- Resolve `std::str::Str` once through the provider and visibility model.
- Assign and carry its ordinary stable `ClassId`.
- Validate declaration kind and the complete representation contract before
  typed lowering.

### Type checking and HIR

- Give every literal the exact resolved language-item type.
- Represent it as a produced value with decoded byte identity, not as an
  initializer or static-method call.
- Apply ordinary expected-type, argument, result, temporary, and assignment
  rules for an exact inline class.
- Never accept another class merely because it is named `Str` or has matching
  fields.

### MIR lowering and verification

- Represent deterministic literal-data identities separately from dynamic
  allocations.
- Materialize all three descriptor fields with explicit complete-object
  construction and publication.
- Use ordinary synthesized `Str` lifecycle and shared-owner operations after
  materialization.
- Represent immortal static shared-array handles explicitly enough to verify
  exact array type, length, complete initialization, and legal ownership use.
- Reject malformed literal descriptors, dynamic publication with the immortal
  tag, mutable static-byte emission, type mismatches, and leaked unpublished
  states.

### Backend

- Pool and emit exact decoded byte blocks deterministically.
- Emit the target's exact shared-array header, metadata reference, length,
  alignment, and immutable data placement.
- Treat `IMMORTAL` retain and release as successful no-ops while preventing a
  dynamic count from entering that state.
- Reuse ordinary class field layout, shared-array metadata, and generated
  ownership helpers.
- Preserve the current generic allocation/free runtime boundary for dynamic
  strings.

No compiler phase may recover the language-item identity from method names,
field spelling alone, or a raw path after canonical resolution.

## Diagnostics

The implementation must provide structured diagnostics for at least:

- an unterminated literal, unescaped newline, direct non-ASCII content,
  unknown escape, or malformed `\xNN`;
- a missing or ambiguous `std::str` module;
- a missing, private, wrong-kind, inherited, or otherwise invalid
  `std::str::Str` declaration;
- any missing, reordered, extra, non-private, or wrongly typed representation
  field;
- a forbidden explicit `Str` copy constructor, assignment, or destructor; and
- string use through the provider-less convenience API.

Language-item diagnostics should identify both the literal use that requires
the item and the declaration or provider evidence that violates the contract.
They must be produced before HIR or backend lowering and must not surface as
host-language exceptions.

## Test obligations

Freezing the proposal does not claim implementation. The later implementation
roadmap must assign these obligations to their owning layers:

- lexer/parser coverage for empty, ASCII, every escape, embedded zero,
  `0x80`/`0xff`, malformed escapes, non-ASCII source, termination, recovery,
  spans, and nesting limits;
- module tests for synthetic reachability, one canonical load, explicit source
  import independence, ambiguity, cycles, `--no-stdlib`, alternate providers,
  privacy, wrong kinds, exact case, and the provider-less API;
- resolution and type-check tests proving exact `ClassId` use, structural
  validation, no leaf-name fallback, produced-value typing, and ordinary
  argument/result/assignment behavior;
- HIR and MIR dump tests for literal identity, descriptor construction,
  lifecycle operations, deterministic pooling, and no method selection;
- MIR mutation tests for malformed static allocation identity, type, length,
  publication, ownership, and immortality state;
- backend tests for exact bytes, embedded zero, alignment, relocations,
  canonical empty backing, deterministic symbols, and the absence of per-use
  allocation or byte copies;
- shared-count tests proving immortal retain/release no-ops, ordinary dynamic
  last-owner behavior, and failure before a dynamic count collides with the
  sentinel;
- native tests for literal length and byte observation, descriptor copying,
  reassignment, shared slicing, dynamic backing reclamation, and repeated
  literal evaluation; and
- runtime-boundary tests proving that no string-specific public C symbol or
  ABI version change was introduced.

The eventual implementation must pass `make check`; Rust changes must also
pass the repository's documented MSRV gate.

## Explicit exclusions

This proposal does not define:

- UTF-8, Unicode scalar values, graphemes, normalization, locale, or collation;
- a separate character type;
- null termination or C-string interoperation;
- mutable string values or mutable views through `Str`;
- compile-time string concatenation or adjacent-literal syntax;
- string operators, interpolation, formatting syntax, or implicit conversion;
- a complete standard-library string or builder API;
- final fields, immutable classes, frozen shared owners, or general constant
  evaluation;
- source-visible static fields, globals, module initialization, or shutdown
  order;
- weak ownership, atomic reference counts, or a threading contract;
- exposing allocation, backing, range, metadata, or count identity;
- external C ABI passage of `Str`; or
- a new C runtime string, array, reference-counting, or interning service.

These features may build on the frozen string model later, but none is
required to freeze or implement this proposal.

## Promotion and implementation ordering

After the three member prerequisites are implemented and the confirmation pass
succeeds:

1. promote source-visible rules into `docs/language/STRINGS.md`;
2. promote phase, immortality, layout, verification, and runtime rules into
   `docs/compiler/STRINGS.md` and the shared-ownership contracts;
3. mark strings as **frozen design** in the status matrix;
4. archive this proposal as the historical design record; and
5. create a PR-sized implementation roadmap without changing the frozen
   representation or literal contract.

The later implementation should proceed from syntax and language-item
resolution through typed production, verified immortal backing, target
emission, and standard-library behavior. That ordering belongs to the
implementation roadmap rather than this design proposal.
