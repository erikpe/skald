# Standard Library

This directory contains the installed Skald standard-library source. The
canonical `std::str::Str` module provides the frozen byte-string descriptor,
safe byte-copying construction, checked observation and slicing, independent
array conversion, read-only structural byte indexing and omitted-bound
descriptor slicing, byte equality against generic objects, concatenation, and
canonical formatting plus optional parsing for every primitive type. Its
integer methods delegate to type-named helpers in
`std::str::format_integer` and `std::str::parse_integer`; the descriptor keeps
no decimal integer algorithm of its own. Its `to_f64` facade recognizes
special values with ordinary string equality and
delegates finite conversion to `std::str::parse_f64`; `from_f64` similarly
keeps special spellings in the facade and delegates finite formatting to
`std::str::format_f64`. The exact standard-stream and primitive-line-output
APIs are implemented in Skald, with no scalar runtime observation surface.

The type-named `std::i64`, `std::u64`, `std::u8`, `std::f64`, and `std::bool`
modules provide explicit `BoxI64`, `BoxU64`, `BoxU8`, `BoxF64`, and `BoxBool`
classes. Every box exposes its exact primitive payload as one public final
`value` field and implements ordinary `Equatable` and `Hashable` interfaces.
The field is directly readable without a getter; replacing a complete mutable
box value remains valid. Integer and boolean boxes use exact primitive
equality; `BoxF64` uses exact binary representation. Each class XORs its `u64`
representation with a distinct fixed domain before `std::hash::mix_u64`, so
same-looking values in different primitive domains do not start from the same
mixer input. Boxing is explicit and adds no compiler or runtime machinery.

The `std::f64` module additionally provides exact `to_bits(f64) -> u64` and
`from_bits(u64) -> f64` value reinterpretation. Its public functions are
ordinary Skald wrappers over two private compiler intrinsics. Typed HIR,
verified MIR, and x86-64 lowering preserve every binary64 bit inline; the
module adds no foreign call or runtime ABI surface. Its inline `BoxF64` class
implements `Equatable` and `Hashable` using the exact binary representation:
signed zeroes and distinct NaN payloads remain distinct, while identical bit
patterns compare equal. Its hash code domain-separates those bits and passes
them through `std::hash::mix_u64` for well-distributed output.

The `std::hash` module provides `mix_u64`, a deterministic SplitMix64 finalizer
over one complete `u64`. Primitive box classes apply their own fixed domain
separation before calling this shared mixer. It is an ordinary
non-cryptographic library function and introduces no runtime or compiler
support.

The `std::io` module has an implemented
[whole-stream source contract](../docs/language/IO.md) and a separate
[compiler/runtime contract](../docs/compiler/IO.md). Its five private
byte-array declarations, compiler HIR/MIR, and x86-64 lowering are implemented,
and runtime ABI version 9 provides independently tested host byte operations.
All nine public functions are ordinary Skald library code over those private
`u8[]` intrinsics and the canonical primitive `Str` conversions. Reads own
geometric buffering, EOF loops, exact-length trimming, normal file close, and
the existing final `Str.from_bytes` copy. The exact conversion surface and text
contract are documented in
[Skald Strings](../docs/language/STRINGS.md#frozen-primitive-textual-conversions)
and its completed rollout is recorded in the archived
[primitive string conversions roadmap](../docs/archive/PRIMITIVE_STRING_CONVERSIONS_ROADMAP.md).

The `std::process` module has an implemented
[process-argument contract](../docs/language/PROCESS.md). Its `args()` function
composes existing `std::io`, raw-byte strings, arrays, loops, and modules to
read the Linux invocation vector. It introduces no compiler intrinsic,
entry-function parameter, static cache, or runtime ABI addition.

The dependency-free `std::lang` module provides the foundational `Equatable`
and `Hashable` interfaces. Equality accepts a read-only `Obj` view so an
implementation can return `false` for values outside its equality domain;
hashing returns one `u64`. Both declarations are ordinary explicitly imported
library interfaces without implicit prelude behavior or compiler machinery.

The dependency-free `std::iter` module provides the canonical generic
`Iterable<Item, State>` interface. Resolution validates its exact public
template and requirement identities when the canonical module is requested.
General `for-in` syntax supplies implicit `std::iter` dependency evidence.
Resolution selects exact nominal applications, including frozen generic
bounds, and the structured HIR lowers through ordinary calls and cyclic MIR.

The `std::range` module imports the foundational iteration and ordering
protocols, provides canonical `Successor<Output>`, and implements ordinary
half-open ascending `Range<T> implements Iterable<T, T>`. Resolution validates
their exact identities when explicitly reachable. Exact classes implement
ordering and successor ordinarily; definition-site generic bound calls over
`u8`, `u64`, and `i64` use compiler-provided static evidence without creating
primitive interface objects or runtime support. Concise range syntax is not
installed yet.

The implemented [operator-overloading contract](../docs/language/OPERATOR_OVERLOADING.md)
assigns the dependency-free `std::ops` module one complete canonical bundle of
generic arithmetic, bitwise, shift, typed-equality, and direct-ordering
interfaces. Ordinary explicit protocol imports or direct entry selection make
it reachable; operator punctuation creates no dependency. Classes can already
use the declarations through ordinary claims, bounds, interface types, and
manual method calls. The complete non-generic punctuation surface selects
eligible class applications when the module is already reachable and erases
them to ordinary interface calls, including ordinary receiver carriers,
result ownership, dispatch, evaluation, cleanup, effects, and target
retention. Definition-site generic selection closes to ordinary class-witness
dispatch or one of the sixty exact compiler-owned primitive evidence cells.
The feature adds no runtime service, primitive object conformance, or ABI
surface.

The `std::vec` module provides the implemented generic
[`Vec<T>` vector](../docs/language/VECTORS.md). It owns independent `T?[]`
capacity storage and infers its element lifecycle requirements from ordinary
method operations. Heterogeneous shared-object code uses `Vec<shared Obj>`.
The vector provides capacity, geometric growth, checked positive and negative
indexing through compatibility methods and structural brackets, independent
logical-length slices, equal-length snapshot slice replacement, push, pop,
last, replacement, clear, and ordinary allocation-free-state
`Iterable<T, u64>` traversal without Vec-specific compiler or runtime
machinery.

The `std::map` module provides a generic `Map<K, V>` whose keys implement both
`Equatable` and `Hashable`. It uses power-of-two open-addressed storage, cached
hashes, linear probing, tombstones, and geometric growth. The map supports
membership testing, checked lookup, insertion and replacement, removal,
clearing, requested capacity, and structural bracket reads and writes without
compiler or runtime machinery.

The `std::error` module declares the compiler-known
`panic(message: std::str::Str) -> unit` intrinsic and imports `std::str`
explicitly. Call statements execute through the compiler's non-returning panic
terminator and the length-delimited runtime reporter. It is not an external
function or an exception API.

The `std::test` module provides ordinary source-level assertions for golden
and application tests. It exports exact equality helpers for `i64`, `u64`,
`u8`, `f64`, and `Str`, boolean truth helpers, and an unconditional `fail`
helper. Assertion failures format values through the canonical `Str`
conversions and terminate through `std::error::panic`; the module adds no
compiler intrinsic or runtime ABI surface. Floating-point equality uses
ordinary exact `f64` equality, so signed zeroes compare equal and NaN values
do not.

`std::str` selectively imports that panic declaration for invalid byte and
slice bounds, forming an ordinary two-module cycle with `std::error`. It also
imports the integer and binary64 formatting and parsing descendant modules in
one direction. The integer helpers depend only on primitive values and arrays.
The binary64 parser and formatter depend only on primitive values and arrays,
and independently import `std::f64` for bit conversion. Their private cached
powers are initialized directly as static `u64[]` fields before execution.

A module and descendant module may coexist: the `std/str.ska` source is
`std::str`, while files below `std/str/` are distinct descendant modules.
`std::str::format_integer` provides `format_i64`, `format_u64`, and
`format_u8`, each returning a fresh exact-length `shared u8[]`.
`std::str::parse_integer` provides the corresponding `parse_i64`, `parse_u64`,
and `parse_u8` functions over validated backing-array ranges. `Str` lends its
private backing only for each parse call; direct users can pass only arrays
they already possess.

The binary64 parser's public
`parse(ref storage: u8[], start: i64, length: u64) -> f64?`
validates its range and is available for direct decimal parsing. `Str.to_f64`
borrows its private backing only for that call; the call-scoped alias cannot
expose the backing to callers. The formatter's public `format(value: f64) ->
shared u8[]` requires a finite value because the `Str` facade owns special
spellings. It uses fixed-width Ryū arithmetic over a reusable private static
cached-power table set and allocates only the returned exact-length array per
value. The public
`BigUnsigned` class now lives in `std::str::parse_f64` beside its only consumer
and remains a narrow parser implementation entry point, not part of the
supported `Str` conversion surface. The parser keeps the existing exact small
path, uses fixed-width Eisel-Lemire conversion for ordinary decimals, and
rescans only ambiguous inputs into a 768-digit exact fallback. Its static
powers and arithmetic helpers are independent from the formatter. Third-party-
derived standard-library code is listed in
[Third-party notices](THIRD_PARTY.md).

Import and call the panic intrinsic as a standalone statement:

```ska
from std::error import panic;

fn main() -> i64 {
    panic("configuration is missing");
}
```

The reporter writes `panic: `, the exact string bytes, and a line feed to
standard error, then terminates unsuccessfully. It does not unwind or run
remaining cleanup.

Named private standard-library members begin with `_` by convention, including
private fields and private instance or static methods. Declarations without a
member name, such as `private init(...)`, are unchanged. Parameters and local
variables use ordinary descriptive names unless they independently require an
underscore.

The public string contract is documented in
[Skald Strings](../docs/language/STRINGS.md). Feature maturity and the
remaining broader library scope are tracked in the
[status matrix](../docs/language/STATUS.md#not-implemented).
