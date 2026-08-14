# Skald Strings

Status: **implemented contract**. The compiler accepts and decodes
string-literal syntax, conditionally discovers and validates
`std::str::Str`, and represents literals as exact typed produced values
through verified MIR descriptor materialization and deterministic x86-64
execution. The installed standard library provides copying construction,
observation, slicing, conversion, and concatenation behavior.
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
    private _storage: shared u8[];
    private _start: i64;
    private _length: u64;

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
0 <= start
(u64) start <= storage.len()
length <= storage.len() - (u64) start
```

The signed start uses the same position type as array and string indices while
the length remains an unsigned count. The standard library is trusted to
preserve the non-negative, in-bounds descriptor invariant. Public source cannot
select the private fields. Interfaces and ordinary instance or static methods
do not alter the representation and remain permitted.

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

Ordinary initializers may be private; copy construction, assignment, and
destruction remain visibility-free lifecycle slots. The canonical `Str`
implementation keeps its empty initializer public and uses a private ordinary
initializer to install a trusted backing owner, start, and length. Dynamic
factories pass fresh backing and its complete range, while slicing passes the
existing backing and a checked subrange. These paths use the ordinary
declaring-class privacy rules; they are not string-specific capabilities.

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

The produced-receiver contract allows a literal or exact `Str`
call result to invoke a read-only method directly:

```ska
var generated: Str = "item-".concat(Str.from_i64(index));
var byte_value: u8 = values.last().byte(byte_index);
```

Both receivers are exact produced `Str` values. Each is completed once
in hidden caller-owned storage before its method arguments and kept live
through the call and result transfer. The current compiler accepts both forms
without staging either receiver in a named `Str` local. No `Str` method name
receives compiler significance from this extension.

Single-quoted [byte literals](TYPES_AND_VALUES.md#literal-types-and-ranges)
reuse the byte-oriented escape vocabulary where applicable but have exact type
`u8` and decode to exactly one byte. They do not construct `Str`, load the
string language item, or imply a character or Unicode model.

## Language-item dependency and source access

A module containing a valid string literal acquires a compiler-owned
dependency on logical module `std::str`. Source does not need an import merely
to evaluate a literal. The dependency participates in ordinary deterministic
provider discovery, ambiguity handling, cyclic graph construction, and
exact-case matching. It may point back to `std::str` itself without creating a
source self-import.

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
| `fn byte(index: i64) -> u8` | Return one checked byte using array index semantics. |
| `fn equals(ref other: Obj) -> bool` | Return whether `other` is a `Str` with an identical byte sequence. |
| `fn slice(start: i64, end: i64) -> Str` | Return an `O(1)` shared-backing half-open slice using array bound semantics. |
| `fn to_bytes() -> u8[]` | Return an independent mutable byte array. |
| `fn concat(ref other: Str) -> Str` | Return fresh backing containing both byte sequences. |
| `static fn from_bool(value: bool) -> Str` | Return canonical lowercase boolean text. |
| `static fn from_i64(value: i64) -> Str` | Return canonical signed decimal text. |
| `static fn from_u64(value: u64) -> Str` | Return canonical unsigned decimal text. |
| `static fn from_u8(value: u8) -> Str` | Return canonical unsigned numeric byte text. |
| `fn to_bool() -> bool?` | Parse exact lowercase boolean text, returning `none` on failure. |
| `fn to_i64() -> i64?` | Parse complete signed decimal text, returning `none` on failure. |
| `fn to_u64() -> u64?` | Parse complete unsigned decimal text, returning `none` on failure. |
| `fn to_u8() -> u8?` | Parse complete unsigned numeric byte text, returning `none` on failure. |
| `fn to_f64() -> f64?` | Parse complete decimal or exact special-value text with correct binary64 rounding, returning `none` on failure. |

Byte indices and slice bounds use the same one-time negative normalization as
array indices and explicit array slice bounds, relative to the current
string's length. Thus `byte(-1)` selects the final byte, and
`slice(1, -1)` excludes the first and final bytes. Slice ranges are half-open:
the normalized start is included and the normalized end is excluded. Both
bounds are required because this method surface has no omitted-argument form.
A valid byte position satisfies `0 <= index < len`; a valid slice satisfies
`0 <= start <= end <= len` after normalization. See
[array length, indices, and bounds](ARRAYS.md#length-indices-and-bounds) and
[array slices](ARRAYS.md#slices).

Invalid byte and slice bounds call the imported
`std::error::panic("array index out of bounds")` declaration as a standalone
non-returning statement. `std::str` therefore has an ordinary explicit import
of `std::error`, while the panic signature gives `std::error` its ordinary
explicit import of `std::str`. This two-module cycle grants no implicit
bindings or visibility exceptions. The library's dynamic factories and
slicing method call a private ordinary initializer to install a trusted
backing owner and range. The initializer is not a compiler convention.

The implemented panic API accepts the exact `std::str::Str` value described here.
Generated code passes only its logical backing-byte address and length to the
[length-delimited reporter](../compiler/RUNTIME_ABI.md#panic-reporting-abi);
the runtime does not receive this descriptor or its shared owner. Failures
encountered while evaluating or copying the message occur before reporting
and use the applicable compiler-known reason from the
[common policy](ERRORS.md#frozen-panic-design).

The required asymptotic behavior is:

| Operation | Required behavior |
|---|---|
| Copy construction | `O(1)` descriptor copy and one shared retain |
| Copy assignment | `O(1)` secure incoming owner, release old owner, copy bounds |
| Destruction | `O(1)` shared release, possibly reclaiming dynamic backing |
| Length | `O(1)` descriptor read |
| Byte access | `O(1)` checked range access |
| Equality | `O(n)` dynamic `Str` check, length check, and byte comparison; no byte copy |
| Slice | `O(1)` owner copy plus adjusted bounds; no byte copy |
| Convert from caller-owned bytes | `O(n)` fresh allocation and byte copy |
| Convert to independent `u8[]` | `O(n)` byte copy |
| Concatenation | `O(n + m)` fresh allocation and byte copies |
| Format a boolean | `O(1)` literal-backed result |
| Format an integer | `O(d)` final-backing allocation and decimal digit emission |
| Format a binary64 value | `O(1)` over the fixed binary64 domain: one exact-length result allocation plus bounded fixed-width Ryū arithmetic over statically initialized cached-power tables |
| Parse a boolean | `O(1)` exact byte comparison with no allocation |
| Parse an integer | `O(n)` checked decimal accumulation with no allocation |
| Parse a binary64 value | `O(n)` allocation-free scan, followed by an exact small conversion or fixed-width Eisel-Lemire conversion in ordinary cases; ambiguous inputs use one `O(n)` rescan plus bounded 768-digit, 4096-bit exact rounding storage and work |

## Frozen primitive textual conversions

Status: **implemented contract**.
This section settles the
standard-library API and portable text contract for conversion between `Str`
and every primitive value type. It adds no language syntax, compiler-known
method, intrinsic, or runtime ABI. The implementation belongs in ordinary
standard-library source and composes the existing primitive operators, arrays,
loops, static methods, and optional results. `Str` owns the user-facing
conversion surface; substantial implementation code lives in focused
companion modules.

Integer facades delegate to the type-named helpers in
`std::str::format_integer` and `std::str::parse_integer`. Formatting helpers
return one fresh exact-length `shared u8[]`, which `Str` immediately adopts.
Parsing helpers accept a validated `(ref storage, start, length)` range and
perform checked decimal accumulation without allocation. The borrowed array
is available only for the helper call and is never returned or retained.

`Str.to_bool` compares directly with the literal-produced `Str` values
`"true"` and `"false"`. `Str.to_f64` likewise recognizes `"NaN"`,
`"Infinity"`, and `"-Infinity"` through direct `Str.equals_str` calls, then
borrows its private backing array into the decimal parser's public
implementation helper
`std::str::parse_f64::parse(ref storage: u8[], start: i64, length: u64) ->
f64?`. Each literal is materialized once in caller-owned full-expression
storage, borrowed without a copy for the comparison, and cleaned immediately
afterward. The parser helper validates the requested range and returns `none`
for invalid bounds or non-decimal text. Its call-scoped read-only alias neither
copies the bytes nor exposes a string's private backing to the caller.

`Str.from_f64` recognizes NaN and both infinities at the facade and returns the
corresponding literal-backed `Str`. Finite values delegate to
`std::str::format_f64::format(value: f64) -> shared u8[]`, whose implementation
contract requires a finite input. The helper returns fresh, exact-length
mutable storage which `Str` immediately adopts behind its private descriptor.
A user calling the helper directly owns only that newly created array; the API
provides no operation for recovering or mutating the backing array of an
existing `Str`.

Binary64 parsing first retains at most 19 significant decimal digits in a
`u64`. Integer-valued inputs that can be scaled without unsigned overflow, and
values whose significand and power of ten are both exactly representable in
binary64, use the exact small path. Other ordinary values use Eisel-Lemire
conversion with portable `64 × 64 -> 128` multiplication and an independent,
statically initialized power-of-five table covering decimal exponents from
-342 through 308. The parser indexes the selected two-word power directly and
performs no table or numeric-storage allocation on that path. When more than
19 digits are significant, the result is accepted
only if converting both adjacent retained-prefix endpoints selects the same
binary64 value. Near-halfway and otherwise ambiguous inputs are rescanned into
the exact-rounding fallback.

Binary64 formatting reads the exact IEEE-754 representation through
`std::f64::to_bits` and applies Ryū's fixed-width nearest/even interval
algorithm. Values in Ryū's small-integer range avoid the cached-power table.
Other finite nonzero values reconstruct the needed power from Ryū's
size-optimized constants using portable `64 × 64 -> 128` limb arithmetic. The
constants occupy five statically initialized `u64[]` tables containing 104
words in total; numeric conversion performs no per-value table allocation.
Reverse normal-return shutdown releases the arrays' backing.

The exact parser fallback retains at most 768 significant digits plus a
nonzero-tail bit. Every binary64 rounding boundary can be decided within that
bound. It uses the checked 128-limb `BigUnsigned` class owned by
`std::str::parse_f64`; the class owns its arrays and has no access to a `Str`
descriptor or backing array. Formatting presentation applies the frozen
plain/scientific threshold directly into one exact-length result backing. No
host formatter, parser call, runtime conversion, or locale state participates
in production.

The supported primitive conversion surface on `Str` is exactly:

```ska
static fn from_bool(value: bool) -> Str;
static fn from_i64(value: i64) -> Str;
static fn from_u64(value: u64) -> Str;
static fn from_u8(value: u8) -> Str;
static fn from_f64(value: f64) -> Str;

fn to_bool() -> bool?;
fn to_i64() -> i64?;
fn to_u64() -> u64?;
fn to_u8() -> u8?;
fn to_f64() -> f64?;
```

The installed library implements all ten formatter and parser methods.

The primitive type is explicit in every method name. There is no overloaded
`from`, generic `parse`, expected-result-type selection, implicit conversion,
or compiler rewrite. In particular, `from_u8` formats the numeric byte value;
it does not create a one-byte string. `from_bytes` and `to_bytes` retain their
existing distinct meanings.

Every `from_<type>` method is total over its primitive input, subject only to
ordinary allocation failure, and returns a logically immutable `Str`.
Every `to_<type>` method examines the receiver's complete byte sequence and
returns a present optional exactly when that sequence satisfies the grammar
and range below. Empty input, malformed input, trailing input, and an
out-of-range mathematical value return `none`. Input content never causes a
panic. The methods do not trim whitespace or accept a prefix, suffix, digit
separator, embedded zero, or locale-specific spelling unless it is explicitly
listed below.

All produced text is ASCII, deterministic, locale-independent, and contains no
line ending. Formatting never emits Skald source suffixes such as `u` or `u8`.
Parsing consumes the entire raw byte string and is independent of source-code
literal tokenization.

### Boolean text

`from_bool(false)` returns `"false"` and `from_bool(true)` returns `"true"`.
`to_bool` accepts exactly those two lowercase spellings. Case variants,
numeric spellings, and surrounding whitespace return `none`.

### Integer text

Integer formatting uses ordinary base-ten notation:

- zero is `"0"`;
- a positive value has no sign and no leading zero;
- a negative `i64` has one leading `-` followed by its magnitude, including
  `"-9223372036854775808"` for `i64` minimum; and
- `u64` and `u8` never have a sign.

Integer parsing accepts one or more ASCII decimal digits. `to_i64` additionally
accepts one leading `-`; `to_u64` and `to_u8` accept no sign. Leading zeroes are
valid, and `"-0"` produces present `i64` zero. A leading `+`, a bare `-`, and
any non-digit byte return `none`. Accumulation is range-checked before an
operation could wrap: `to_i64` accepts exactly the mathematical interval
`[-9223372036854775808, 9223372036854775807]`, `to_u64` accepts
`[0, 18446744073709551615]`, and `to_u8` accepts `[0, 255]`.

### Floating-point text

`from_f64` preserves the distinctions that are portable in the current
binary64 value model and uses these special spellings:

| Value category | Result |
|---|---|
| positive zero | `"0.0"` |
| negative zero | `"-0.0"` |
| positive infinity | `"Infinity"` |
| negative infinity | `"-Infinity"` |
| any NaN | `"NaN"` |

For a finite nonzero value, `from_f64` emits the shortest decimal significand
that parses, under the rule below, to the same IEEE-754 binary64 value. If
several shortest significands round to that value, it selects the one nearest
the exact mathematical value; an exact tie selects an even final digit. This
choice is deterministic and independent of the host formatting library.

Let `e` be the scientific decimal exponent of that selected significand. For
`-3 <= e < 7`, formatting uses ordinary decimal notation. Otherwise it uses
scientific notation with one digit before the point, an uppercase `E`, and a
base-ten exponent with a leading `-` only when negative and no leading zeroes.
Both forms always contain a decimal point and at least one digit after it;
zeroes needed to place the point are retained, while no other insignificant
zero is added. Examples of the shape are `"1.0"`, `"0.001"`, `"9999999.0"`,
`"1.0E7"`, and `"1.25E-4"`.

`to_f64` accepts the exact special spellings `"NaN"`, `"Infinity"`, and
`"-Infinity"`. It otherwise accepts this ASCII decimal grammar, where `digit`
is `0` through `9` and `digits` is one or more digits:

```text
decimal = ["-"] (digits ["." [digits]] | "." digits)
          [("e" | "E") [("+" | "-")] digits]
```

Thus the parser accepts useful noncanonical forms such as `"1"`, `"01.0"`,
`".5"`, `"1."`, and `"1e+2"`, but accepts no leading `+`, whitespace, source
suffix, hexadecimal form, or case variant of a special spelling. A decimal is
rounded once to nearest binary64 with ties to an even significand. Finite
decimal text that rounds to an infinity returns `none`; gradual underflow to a
subnormal or signed zero is a present result. Exponent accumulation must not
wrap regardless of input length.

The following round trips are guaranteed:

- `Str.from_bool(value).to_bool()! == value` for every `bool`;
- the corresponding format-then-parse equality for every integer value;
- bit-identical format-then-parse results for every finite `f64`, including
  the sign of zero, and the same infinity for either infinity; and
- a NaN result from parsing the formatter's `"NaN"`, without a promise about
  NaN sign, payload, or signaling state.

Parsing does not expose an error category or stopping offset. A later broader
parsing API may add diagnostics or configurable grammars under different
names, but it must not change these methods' accepted language or optional
failure behavior.

The implemented [standard I/O API](IO.md) deliberately composes these existing
operations. Its implemented reads accumulate an ordinary `u8[]` and call
`Str.from_bytes`, while its implemented writes call `to_bytes` before crossing
a private byte-array intrinsic boundary. It adds no borrowed byte view, owned-backing
adoption, builder, primitive conversion, or string-specific runtime ABI. The
extra `O(n)` copies are therefore part of the initial I/O cost model. The
private intrinsic pipeline, lower-phase execution, and four public functions
are implemented.

The broader public method and builder APIs remain standard-library design.
Indexing,
slicing, equality, comparison, hashing, concatenation, formatting, and parsing
are not string-specific operators in this contract. The frozen primitive
conversion methods above are ordinary methods rather than operators or
compiler conversions. In particular, `+` is not
lowered by searching for a method named `concat`. Public checked byte/range
methods implement their array-compatible normalization through ordinary
general primitive comparison, arithmetic, and total conversion operations.
The backing-array maximum-length rule makes the descriptor length and every
valid absolute position representable as `i64`. The compiler adds neither a
checked cast nor string-only numeric rules.

## Exclusions

This frozen profile does not define:

- Unicode or a character type (single-quoted byte literals remain exact `u8`);
- null termination or C-string interoperation;
- mutable strings or mutable views through `Str`;
- adjacent literals, interpolation, formatting syntax, or string operators;
- compile-time concatenation or general constant evaluation;
- a complete `Str`/builder API beyond the frozen primitive conversions;
- `final` fields, immutable classes, or frozen shared-owner types;
- source-visible static fields, globals, or module initialization/shutdown;
- weak ownership, atomic counts, or threading;
- public backing, metadata, count, or allocation identity;
- external C ABI passage of `Str`; or
- a string-specific C runtime service.

Implementation coverage and inspection workflows are documented by
[Testing](../development/TESTING.md#string-coverage) and
[Debugging the Compiler](../development/DEBUGGING.md#string-pipeline-inspection).
