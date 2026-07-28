# Skald Strings

Status: **frozen design, partially implemented through STR4**. The compiler
accepts and decodes string-literal syntax, conditionally discovers and
validates `std::str::Str`, and represents literals as exact typed produced
values through verified MIR descriptor materialization and deterministic
x86-64 execution. The installed standard library provides representative
construction, observation, slicing, conversion, and concatenation behavior.
This document is authoritative for the source-visible string contract, while
the [status matrix](STATUS.md) remains authoritative for compiler availability.

The [compiler string contract](../compiler/STRINGS.md) owns language-item
discovery, intrinsic materialization, immortal backing, verification, and
target realization. General class, ownership, array, module, and evaluation
rules remain owned by their focused language documents.

## Value model

Every string is a finite sequence of `u8` bytes. Skald assigns no Unicode,
UTF-8, character, locale, collation, or normalization meaning to those bytes.
A zero byte is ordinary content, and strings have no required terminator.

The exact language-facing type is the public class `Str` in logical module
`std::str`, with canonical declaration path:

```text
std::str::Str
```

The path is case-sensitive. No local, imported, or unrelated declaration named
`Str` receives string semantics. A class derived from the canonical `Str` is
an ordinary distinct class; only the exact language-item identity is the type
of a literal.

A `Str` is a logically immutable inline descriptor with exactly these first
three direct fields, in order, and no additional direct fields:

```ska
public class Str {
    private storage: shared u8[];
    private start: u64;
    private length: u64;

    // At least one safe ordinary initializer and the library's methods.
}
```

The class has no direct base and declares no explicit copy constructor, copy
assignment, or destructor. Ordinary synthesized field-wise lifecycle applies:
copying retains the shared array owner and copies the bounds, assignment
securely replaces that owner and copies the bounds, and destruction releases
the owner.

Every valid descriptor satisfies:

```text
start <= storage.len()
length <= storage.len() - start
```

The standard library is trusted to preserve this invariant. Public source
cannot select the private fields. Interfaces and ordinary instance or static
methods do not alter the representation and remain permitted.

The source contract does not freeze physical size, alignment, field offsets,
shared-header layout, or literal-data placement.

## Logical immutability

An existing `Str` value's observable byte sequence never changes. A variable
owning a `Str` remains assignable to another complete `Str` value.

Logical immutability is established by:

- private descriptor fields;
- no public API that exposes the backing owner or a mutable byte alias;
- no public mutable-receiver method that changes an existing string's range or
  bytes;
- immutable compiler-emitted literal backing;
- fresh unaliased backing for dynamically frozen caller content; and
- trusted standard-library code that does not mutate storage reachable from a
  live `Str`.

Immortal ownership controls lifetime, not mutability. A public initializer or
factory accepting caller-owned bytes must copy them into fresh storage rather
than retain mutable caller backing. A mutable builder similarly freezes by
copying.

Skald lifecycle declarations have no private visibility. Trusted slicing
therefore does not rely on a private initializer: a private instance or static
helper may copy an already valid `Str` descriptor and adjust its private
bounds. A helper may install backing only when that backing is freshly created
and owned by trusted standard-library code. These paths use the implemented
`private fn`, `static fn`, and `private static fn` rules; private static
methods are ordinary composition, not a distinct capability.

## String literals

A string literal is delimited by double quotes and has the exact static type
`std::str::Str`.

Unescaped content is restricted to printable ASCII bytes other than `"` and
`\`. All other bytes use escapes:

| Escape | Produced byte |
|---|---:|
| `\"` | `0x22` |
| `\\` | `0x5c` |
| `\n` | `0x0a` |
| `\r` | `0x0d` |
| `\t` | `0x09` |
| `\0` | `0x00` |
| `\xNN` | the byte denoted by exactly two case-insensitive hexadecimal digits |

Examples:

```ska
"hello"
"line one\nline two"
"\x00\x7f\x80\xff"
""
```

Literal length is the decoded byte count. Equal decoded byte sequences are
equal contents regardless of escape spelling. Unknown escapes, incomplete
hexadecimal escapes, direct non-ASCII content, unescaped newlines, and
unterminated literals are syntax errors. Recovery does not manufacture a
valid string expression from malformed content.

Literal evaluation produces one exact inline `Str` descriptor whose storage
owns immutable program-lifetime bytes, whose start is zero, and whose length
is the decoded byte count. It does not allocate dynamic byte storage, copy
literal bytes, call an initializer or method, or expose its backing owner.
Ordinary destination, copy, assignment, result, argument, temporary, and
cleanup rules apply to the produced descriptor. Ownership operations on its
immortal backing have no observable effect.

An implementation may pool equal decoded byte sequences and should use one
canonical empty backing per program. Backing identity is unobservable, so the
language does not promise whether distinct occurrences share storage.

## Language-item dependency and source access

A module containing a valid string literal acquires a compiler-owned
dependency on logical module `std::str`. Source does not need an import merely
to evaluate a literal. The dependency participates in ordinary deterministic
provider discovery, ambiguity handling, cycle detection, and exact-case
matching.

The dependency grants only compiler access to validate and materialize the
language item. It creates no unqualified binding, import, or re-export and
does not grant source access to private fields. Source that explicitly names
`Str` or calls its static methods still requires ordinary direct imports and
top-level/member visibility.

`--no-stdlib` remains valid when another configured provider supplies exactly
one conforming `std::str` module. A program without literals does not acquire
the dependency. The provider-less source-text convenience API cannot invent a
built-in `Str`; until it has an explicit provider-aware request, it diagnoses a
missing language item for string use.

## Standard-library boundary and costs

Literal recognition and descriptor materialization are compiler intrinsic.
String operations are ordinary Skald standard-library code. The compiler does
not select an initializer, factory, `from_bytes`, `concat`, or any other method
by spelling.

The installed representative public surface is:

| Member | Behavior |
|---|---|
| `init()` | Construct an empty dynamic string. |
| `static fn from_bytes(ref bytes: u8[]) -> Str` | Copy caller bytes into fresh shared storage. |
| `fn len() -> u64` | Return the descriptor length. |
| `fn byte(index: u64) -> u8` | Return one checked byte. |
| `fn slice(start: u64, length: u64) -> Str` | Return an `O(1)` shared-backing slice after checked bounds validation. |
| `fn to_bytes() -> u8[]` | Return an independent mutable byte array. |
| `fn concat(ref other: Str) -> Str` | Return fresh backing containing both byte sequences. |

Invalid byte and slice bounds terminate through ordinary checked array
behavior. The library uses private static helpers to install newly allocated
backing and to adjust copied descriptors; neither helper is a compiler
convention.

The required asymptotic behavior is:

| Operation | Required behavior |
|---|---|
| Copy construction | `O(1)` descriptor copy and one shared retain |
| Copy assignment | `O(1)` secure incoming owner, release old owner, copy bounds |
| Destruction | `O(1)` shared release, possibly reclaiming dynamic backing |
| Length | `O(1)` descriptor read |
| Byte access | `O(1)` checked range access |
| Slice | `O(1)` owner copy plus adjusted bounds; no byte copy |
| Convert from caller-owned bytes | `O(n)` fresh allocation and byte copy |
| Convert to independent `u8[]` | `O(n)` byte copy |
| Concatenation | `O(n + m)` fresh allocation and byte copies |

The broader public method and builder APIs remain standard-library design.
Indexing,
slicing, equality, comparison, hashing, concatenation, formatting, and parsing
are not string-specific operators in this contract. In particular, `+` is not
lowered by searching for a method named `concat`. Public checked byte/range
methods use ordinary general primitive comparison, arithmetic, and conversion
operations. They compare their `u64` bounds before converting a successful
position to the array index type. The array maximum-length rule and descriptor
invariant make every successful position representable as `i64`; values above
`i64::MAX` fail the unsigned range comparison before the total cast is used.
The compiler adds neither a checked cast nor string-only numeric rules.

## Exclusions

This frozen profile does not define:

- Unicode or a character type;
- null termination or C-string interoperation;
- mutable strings or mutable views through `Str`;
- adjacent literals, interpolation, formatting syntax, or string operators;
- compile-time concatenation or general constant evaluation;
- a complete `Str`/builder API;
- `final` fields, immutable classes, or frozen shared-owner types;
- source-visible static fields, globals, or module initialization/shutdown;
- weak ownership, atomic counts, or threading;
- public backing, metadata, count, or allocation identity;
- external C ABI passage of `Str`; or
- a string-specific C runtime service.

Implementation ordering and acceptance coverage are tracked by the active
[string implementation roadmap](../roadmaps/STRINGS_ROADMAP.md).
