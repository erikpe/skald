# Strings Compiler Contract

Status: **frozen design, implemented through STR1**. The lexer, AST, and module
graph implement literal recognition, decoded bytes, and conditional
`std::str` reachability. Resolution validates the exact language item, and HIR
represents literals as intrinsic produced `Str` values with deterministic data
identities. MIR materialization, immortal allocation, backend emission, and
execution remain future work. This document is authoritative for compiler
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
- composed of exactly three direct private fields, in order:
  `storage: shared u8[]`, `start: u64`, and `length: u64`;
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

The complete compiler driver currently stops after this typed boundary with
structured diagnostic `CMP001`; it does not pass literal-bearing HIR to MIR
lowering until STR2 defines and verifies descriptor materialization.

### MIR and verification

MIR gives decoded byte blocks deterministic program-local identities distinct
from dynamic allocations. Literal materialization:

1. obtains one verified immortal `shared u8[]` handle for the decoded block;
2. initializes the selected private `storage`, `start`, and `length` fields;
3. publishes one complete exact `Str` result; and
4. continues through ordinary synthesized lifecycle and owner operations.

The verifier checks literal-data identity and density, exact array element
type and length, immutable byte payload, one valid metadata identity,
language-item/field identity, complete descriptor initialization, static
publication, and every later ownership use. Dynamic allocation publication
cannot manufacture immortality, and malformed literal descriptors or leaked
unpublished states fail verification.

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
- retaining `u64::MAX - 1` terminates before producing the reserved value; and
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

The current compiler instead treats `u64::MAX` as retain overflow. Implementing
strings deliberately changes generated retain/release behavior only after MIR
can prove the handle's legal immortal origin.

## Compiler and standard-library boundary

The compiler owns:

- literal syntax, decoded bytes, and exact language-item dependency;
- structural language-item validation;
- identity-selected descriptor materialization;
- immortal allocation verification and target emission; and
- generic generated ownership behavior.

The standard library owns range preservation, public constructors/factories,
slicing, byte access, conversion, concatenation, and the broader API. It may
use ordinary public static factories and private instance/static helpers.
Because lifecycle visibility is not private, caller-provided mutable bytes are
accepted only through copying APIs; trusted slices copy an existing descriptor
and update private bounds. No method spelling is compiler-selected.
Checked public range APIs may depend on future general primitive
comparison/conversion support, but receive no string-specific numeric
intrinsics.

Dynamic strings use ordinary shared-array allocation and the existing generic
allocator/free boundary. No public runtime symbol, runtime ABI version, native
string object, interning service, or external `Str` calling convention is
added.

## Diagnostics and test obligations

Structured diagnostics cover:

- every malformed literal category and recovery boundary;
- missing, ambiguous, unreadable, malformed, cyclic, or exact-case language
  item modules;
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
- module tests for synthetic reachability, provider permutations, cycles,
  imports, replacement roots, `--no-stdlib`, and the source-text adapter;
- resolver/type-check tests for exact identities, validation, produced-value
  typing, destinations, calls, and lifecycle;
- HIR/MIR dump and verifier-mutation tests for deterministic data identities,
  descriptor construction, static publication, and immortality;
- backend tests for bytes, zeroes, alignment, relocations, pooling, symbols,
  count boundaries, and absence of allocation/copy calls;
- native goldens for observation, copying, assignment, slicing, dynamic
  reclamation, and repeated literals; and
- runtime-boundary tests confirming the public C surface and ABI version are
  unchanged.

The active [string implementation roadmap](../roadmaps/STRINGS_ROADMAP.md)
assigns these obligations to reviewable tasks.
