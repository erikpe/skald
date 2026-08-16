# Strings Compiler Contract

Status: **implemented contract**. The lexer, AST, and module
graph implement literal recognition, decoded bytes, and conditional
`std::str` reachability. Resolution validates the exact language item, and HIR
represents literals as intrinsic produced `Str` values with deterministic data
identities. MIR declares and verifies immutable immortal backing and exact
descriptor publication. The x86-64 backend emits deterministic immutable
backing and literals execute through ordinary class lifecycle. The canonical
standard-library module implements representative dynamic string behavior
entirely through ordinary class, array, and ownership operations. This
document is authoritative for compiler
handling of the source-visible [string contract](../language/STRINGS.md).

The generic ownership header and generated count machinery are owned by
[Shared Ownership](SHARED_OWNERSHIP.md). Module-provider behavior is owned by
[the module-system contract](MODULE_SYSTEM.md), and target-independent phase
boundaries are owned by [Phases and IR](PHASES_AND_IR.md).

## Canonical language item

The compiler resolves the exact logical declaration path `std::str::Str` once
to an ordinary stable `ClassId`. Lower phases retain that identity and never
repeat leaf-name, method-name, field-name, or raw path comparisons.

The selected declaration must be:

- a public concrete class declared directly as `Str` in module `std::str`;
- without a direct base;
- composed of exactly four direct private fields, in order:
  `_storage: shared u8[]`, `_start: i64`, `_length: u64`, and
  `private cell _hash_code: u64?`;
- without an explicit copy constructor, copy assignment, or destructor; and
- eligible for ordinary synthesized field-wise lifecycle.

Interface conformances and ordinary/static methods are permitted and do not
participate in language-item identity. Another structurally equal class is not
the language item.

Language-item validation occurs while resolved declarations still carry
visibility, source spans, field identities, and declaring-class ownership.
This is compiler metadata inspection, not source member selection. Future HIR
literal construction names validated `ClassId`/`FieldId` identities directly;
it does not grant source access, add a binding, or weaken the declaring-class
privacy rule.

## Conditional module discovery

The module graph loader parses each reached module while discovering explicit
imports. A parsed module containing at least one valid literal contributes one
synthetic dependency on
`std::str`, retaining deterministic literal evidence for diagnostics. Invalid
literal syntax contributes no usable expression or synthetic dependency.

The dependency uses ordinary provider candidate resolution and canonical
module identity:

- the target is loaded at most once;
- missing, ambiguous, unreadable, malformed, cyclic, and exact-case failures
  stay structured loader diagnostics;
- provider order does not break ambiguity;
- explicit imports and the synthetic dependency coalesce to one graph node;
- `--no-stdlib` may use another configured provider; and
- modules without literals add no dependency.

The synthetic edge affects reachability but creates no source import binding.
The provider-less source-text adapter has no hidden standard-library context
and reports structured diagnostic `RES032` that the language item cannot be
provided. Provider-aware resolution reports `RES033` when the reached
canonical declaration is missing, has the wrong kind or visibility, inherits,
has the wrong representation, or declares forbidden lifecycle members.

## Phase responsibilities

### Lexing and parsing

The lexer recognizes a double-quoted token with its complete source span and
decodes exactly the escapes frozen by the language contract. It reports
unterminated content, unescaped newlines, direct non-ASCII bytes, unknown
escapes, and malformed `\xNN` without deferring byte conversion. The AST
retains decoded bytes and source identity in a dedicated expression node.

Parser recovery resumes at an appropriate expression, statement, declaration,
or resource-limit boundary and never turns malformed content into a valid
literal.

### Resolution and HIR

Resolution validates the canonical class contract before typed lowering and
records the exact class identity on every literal. One invalid language item
may diagnose all relevant declaration/provider evidence without repeating
structural analysis per occurrence.

HIR represents a literal as a produced exact-class value plus deterministic
decoded-data identity. It is not a construction, initializer call, static
call, allocation call, or opaque standard-library invocation. Ordinary
expected-type, destination, argument, result, temporary, copy, assignment, and
cleanup machinery handles the resulting `Str`.

Literal-bearing HIR passes through MIR lowering, verification, target
selection, and assembly emission. No driver-only feature gate remains.

The produced exact-class receiver contract treats a literal or
`Str`-returning call as an ordinary exact-class producer when it is followed
by a read-only instance method. It adds no string-specific AST or HIR node and
does not make any method name compiler-known. The compiler lowers that
receiver through the ordinary produced-object temporary path; the shared phase
representation is defined in
[Compiler Phases and Intermediate Representations](PHASES_AND_IR.md#produced-exact-class-method-receiver-representation).

Produced-object field reads use that same path. Within the declaring `Str`
class, a closed-generic `Vec<Str>` getter result may project private descriptor
fields directly; `Str.join` uses this for its length pass, and `to_bool` reads
the private length of literal producers before comparison. Resolution still
applies declaring-class privacy, and HIR/MIR reuse ordinary produced views,
field projections, owner operations, logical lifetimes, and cleanup without a
string-specific carrier.

### MIR and verification

MIR gives decoded byte blocks deterministic program-local identities distinct
from dynamic allocations. Literal materialization:

1. obtains one verified immortal `shared u8[]` handle for the decoded block;
2. initializes the selected private `_storage`, `_start`, and `_length` fields
   and sets the selected private-cell `_hash_code` field absent;
3. publishes one complete exact `Str` result; and
4. continues through ordinary synthesized lifecycle and owner operations.

The verifier checks literal-data identity and density, exact array element
type and length, immutable byte payload, one valid metadata identity,
all four language-item field identities, absent initial hash state, complete
descriptor initialization, static
publication, and every later ownership use. Dynamic allocation publication
cannot manufacture immortality, and malformed literal descriptors or leaked
unpublished states fail verification.

Static backing production is a distinct MIR operation, not a dynamic
allocation with a special count. It produces one short-lived exact
`shared u8[]` owner that only the matching descriptor publication may consume.
The completed `Str` then uses the ordinary class copy, assignment, result,
temporary, and cleanup machinery.

### Backend

The x86-64 backend pools decoded byte blocks deterministically, emits stable
collision-proof private symbols, and places bytes and relocations in immutable
or relocation-read-only program data. Each backing uses the existing exact
shared-array layout:

| Offset | Width | Meaning |
|---:|---:|---|
| 0 | 8 | immortal strong-count sentinel |
| 8 | 8 | exact `u8[]` metadata/finalizer-table pointer |
| 16 | 8 | decoded byte length |
| 24 | length | decoded bytes |

Ordinary target alignment and padding follow the array layout owner. Literal
evaluation emits no dynamic allocator call and copies no bytes. The backend
uses ordinary class layout to address descriptor fields and ordinary generated
ownership helpers after materialization.

Other targets may use a different representation while preserving the
language-visible value, ownership, and immutability contract.

## Immortal shared storage

Immortality is a compiler-private state of an exact shared allocation, not a
string-only array kind or source qualifier. The frozen extension reserves:

```text
IMMORTAL = u64::MAX
```

for verified program-lifetime storage. Generated operations behave as follows:

- retaining or releasing `IMMORTAL` succeeds without storing, finalizing, or
  freeing;
- ordinary positive dynamic counts retain and release normally;
- retaining `u64::MAX - 1` reports ownership-count exhaustion before producing
  the reserved value; and
- zero remains invalid for non-optional handles and remains the optional-owner
  absence niche.

Only a verified compiler static-allocation producer may publish the sentinel.
Ordinary source `new`, dynamic arrays, generated publication, and the C runtime
cannot create it. MIR and backend legality keep static immortal allocation
distinct from count-one dynamic publication.

The state is generic enough for other exact shared layouts, but this contract
authorizes its first producer only for verified string-literal `u8[]` backing.
It does not define globals, static fields, static initialization, or immortal
mutable source storage.

The compiler implements this count behavior generically. MIR verification is
the authority that restricts sentinel publication to the exact supported
static producer; generated dynamic publication still writes one.

## Compiler and standard-library boundary

The compiler owns:

- literal syntax, decoded bytes, and exact language-item dependency;
- structural language-item validation;
- identity-selected descriptor materialization;
- immortal allocation verification and target emission; and
- generic generated ownership behavior.

The standard library owns range preservation, public constructors/factories,
slicing, byte access, conversion, concatenation, and the broader API. It may
use ordinary public static factories, private initializers, and private
instance/static helpers. The canonical implementation retains its public empty
initializer and initializes one ordinary private static `_EMPTY_STORAGE` owner
with `new u8[]()`. Default descriptors copy this shared owner, so repeated
`Str()` construction performs no per-instance allocation; reverse normal-return
static shutdown releases the root owner. One private ordinary initializer
accepts a trusted backing owner, start, and length. Caller-provided mutable
bytes are accepted only through copying APIs; trusted slices pass an existing
backing and checked subrange to that initializer. No initializer, field, or
method spelling is compiler-selected.
Checked public range APIs accept exact `i64` positions and implement the same
one-time negative normalization as arrays, relative to the descriptor length.
Every backing-array length is at most `i64::MAX`, so converting the descriptor
length to `i64` before adding a negative bound cannot overflow, including for
`i64::MIN`. The checked non-negative normalized position and the descriptor
start are both `i64`, so deriving an absolute backing position needs no numeric
conversion. The descriptor invariant keeps the resulting absolute position
within the backing. No checked cast or string-specific numeric intrinsic is
required.

Dynamic strings and the private default-empty static use ordinary shared-array
allocation and the existing generic allocator/free boundary. No public runtime
symbol, runtime ABI version, native string object, interning service, or
external `Str` calling convention is added.

Runtime ABI version 9 includes one common length-delimited reporter, not a
string ABI. Panic lowering validates the canonical `Str` identity, then
extracts its logical backing-byte address and length through the existing
field identities and target layout. The C reporter never receives a
descriptor or owner, and this document does not duplicate the
[static failure catalog](../language/ERRORS.md#frozen-panic-design).

The installed `std/std/str.ska` implementation copies caller arrays into fresh
shared backing, observes length and checked bytes, creates `O(1)` slices by
passing shared backing and a checked subrange to a private initializer,
converts to an independent inline array, and concatenates into fresh backing.
Its invalid byte and slice checks lower ordinary calls to the selectively
imported canonical panic identity using the frozen `array index out of bounds`
message. The reciprocal `std::str -> std::error` and
`std::error -> std::str` imports are an ordinary module cycle; no string,
panic, or standard-library exception exists in graph loading or resolution.
Its dynamic factories pass fresh shared backing and the complete range to the
same initializer under exact-class lexical ownership. Synthesized class
lifecycle and generic shared-array retain/release reclaim dynamic backing
after its last descriptor owner.

## Diagnostics and test obligations

Structured diagnostics cover:

- every malformed literal category and recovery boundary;
- missing, ambiguous, unreadable, malformed, wrong-case, or directly
  self-importing language-item modules;
- private, missing, wrong-kind, inherited, or otherwise invalid `Str`;
- missing, reordered, extra, public, or wrongly typed representation fields;
- forbidden explicit copy/assignment/destruction lifecycle;
- provider-less source-text use; and
- invalid MIR literal, descriptor, static allocation, or ownership state.

Diagnostics connect the requiring literal with declaration or provider
evidence and appear before backend lowering.

Implementation coverage belongs at the narrowest owner:

- lexer/parser tests for bytes, escapes, errors, spans, recovery, and resource
  limits;
- module tests for synthetic reachability, provider permutations, cyclic
  dependencies, imports, replacement roots, `--no-stdlib`, and the source-text
  adapter;
- resolver/type-check tests for exact identities, canonical cyclic imports,
  validation, produced-value typing, destinations, calls, and lifecycle;
- HIR/MIR dump and verifier-mutation tests for deterministic data identities,
  descriptor construction, static publication, and immortality;
- backend tests for bytes, zeroes, alignment, relocations, pooling, symbols,
  count boundaries, and absence of allocation/copy calls;
- native goldens for observation, copying, assignment, slicing, dynamic
  reclamation, and repeated literals; and
- runtime-boundary tests confirming the public C surface and ABI version are
  unchanged.

The [testing guide](../development/TESTING.md#string-coverage) maps these
obligations to their current owners, and the
[debugging guide](../development/DEBUGGING.md#string-pipeline-inspection)
describes the corresponding phase products.
