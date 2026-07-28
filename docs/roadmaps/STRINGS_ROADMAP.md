# String Types Implementation Roadmap

Status: planned; STR0 is next.

This roadmap implements the frozen
[string language contract](../language/STRINGS.md) and
[compiler contract](../compiler/STRINGS.md) without turning `Str` into a
built-in nominal type, introducing method-name conventions, or widening the C
runtime. It proceeds from conditional language-item discovery and literal
syntax through exact typed identity, verified immortal backing, target
emission, and ordinary standard-library behavior.

## Scope and invariants

- The exact string type is the public `std::str::Str` class with the frozen
  private three-field descriptor and synthesized lifecycle.
- Literal syntax decodes bytes before HIR and conditionally reaches the exact
  language-item module through ordinary provider resolution.
- Resolution assigns one ordinary `ClassId`; lower phases never compare
  `Str`, field, initializer, or factory spellings.
- Literal HIR/MIR is an intrinsic produced exact-class value, never an
  initializer or static-method call.
- Immortality is an explicit verified shared-allocation origin. Dynamic
  allocation cannot manufacture the sentinel.
- Literal evaluation performs no dynamic allocation or byte copy. Descriptor
  ownership and cleanup reuse ordinary class/shared machinery.
- Dynamic string operations are ordinary Skald standard-library source using
  safe public construction and private instance/static helpers.
- No string-specific public C runtime symbol, ABI version, external `Str`
  calling convention, static field, global, or initialization-order contract
  is added.
- Diagnostics, decoded-data identities, pooling, dumps, symbols, provider
  behavior, and native observations remain deterministic.
- Unicode, character values, interpolation, operators, general constant
  evaluation, a complete builder API, and external string interoperation are
  non-goals.

## Progress

- [ ] STR0 — Recognize literals and discover the canonical language item
- [ ] STR1 — Validate and type intrinsic `Str` production
- [ ] STR2 — Verify literal descriptors and immortal shared backing
- [ ] STR3 — Emit deterministic immortal literal data on x86-64
- [ ] STR4 — Provide ordinary standard-library string behavior
- [ ] STR5 — Harden, document, and promote strings as implemented

## PR-sized implementation sequence

### STR0 — Recognize literals and discover the canonical language item

**Purpose:** Establish the complete source and module reachability boundary
before later phases depend on string identity.

- [ ] Add a dedicated lexer token and AST expression carrying decoded bytes,
      full source span, and deterministic syntax dump output.
- [ ] Implement printable-ASCII content and the exact frozen escapes. Diagnose
      unknown/incomplete escapes, non-ASCII content, unescaped newlines, and
      unterminated literals without manufacturing a valid expression.
- [ ] Keep nesting/resource limits and parser recovery deterministic at
      expression, statement, and declaration boundaries.
- [ ] Extend the module graph loader's parsed dependency discovery so any
      module containing a valid literal contributes one synthetic
      `std::str` dependency with requiring-literal evidence.
- [ ] Coalesce synthetic and explicit reachability by canonical module path
      while preserving ordinary ambiguity, exact-case, unreadable-source,
      malformed-source, cycle, provider-order, replacement-root, and
      `--no-stdlib` behavior.
- [ ] Ensure the synthetic dependency creates no source binding and modules
      without literals remain independent of `std::str`.
- [ ] Give the provider-less source-text adapter one structured
      missing-language-item result instead of synthesizing a built-in class.
- [ ] Keep `module/graph/mod.rs`, `syntax/mod.rs`, and other facades concise;
      place literal scanning/decoding and synthetic-dependency policy with
      their cohesive owners.

**Tests:** Lexer/parser byte, escape, span, dump, invalid-content, recovery,
and resource-limit tests; graph/provider matrices for one canonical load,
explicit-import coalescing, ambiguity, cycles, exact case, replacement and
disabled standard-library roots, source order, and provider-less use;
compile-fail goldens; `make check`, `make msrv-check`,
`make robustness-long`, and `git diff --check`.

**Exit criteria:** Valid literal syntax is represented losslessly and reaches
exactly one canonical `std::str` module when provider context exists, while no
literal is yet accepted into typed HIR.

### STR1 — Validate and type intrinsic `Str` production

**Purpose:** Resolve the language item once and introduce string literals at
the typed identity boundary without method-name or layout reconstruction.

- [ ] Add a resolution-owned language-item result that records the canonical
      `ClassId`, three selected `FieldId` values, declaration evidence, and
      requiring literal spans.
- [ ] Validate public exact module ownership, root-class shape, exact ordered
      private field types/count, and absence of explicit copy, assignment, and
      destruction lifecycle before HIR.
- [ ] Report missing/private/wrong-kind/inherited declarations and every
      structural mismatch with declaration-linked deterministic diagnostics.
- [ ] Treat private-field inspection as compiler metadata validation only;
      ordinary source selection continues through the existing
      declaring-class privacy boundary.
- [ ] Add resolved and HIR literal nodes carrying exact class and decoded-data
      identity. Do not encode a constructor, initializer, factory, or static
      call.
- [ ] Integrate literals as produced exact-class values through expected-type,
      local/field initialization, arguments, results, assignment,
      conditionals, temporaries, and ordinary synthesized lifecycle.
- [ ] Allocate literal-data identities in deterministic canonical order and
      expose them only through narrow resolved/HIR facades and dumps.

**Tests:** Resolver structural-validation and exact-identity tests, including
same-leaf and structurally equal impostors; visibility/privacy non-escape;
type-check/HIR destination, argument, result, copy, assignment, and temporary
tests; exact diagnostics and dumps; module/process determinism; focused
goldens; repository gates.

**Exit criteria:** Every valid literal becomes a produced exact canonical
`Str` HIR value, and all invalid language items fail before HIR without any
method-name convention.

### STR2 — Verify literal descriptors and immortal shared backing

**Purpose:** Make target-independent literal materialization and lifetime
state explicit before a backend can emit static storage.

- [ ] Add deterministic MIR literal-data declarations with exact decoded
      bytes, `u8[]` target identity, length, and static immortal origin.
- [ ] Lower a literal into complete identity-selected initialization of
      `storage`, `start`, and `length`, followed by ordinary `Str` lifecycle.
- [ ] Introduce an explicit static-allocation producer distinct from dynamic
      unpublished allocation and count-one publication.
- [ ] Extend shared-owner copy, assignment, result, temporary, cleanup, and
      anchor effects without adding a string-specific ownership path.
- [ ] Verify data-table density/identity, immutable payload, exact array
      metadata and length, legal descriptor fields, one complete publication,
      ownership accounting, and absence of dynamic immortal publication.
- [ ] Reject malformed descriptors, mismatched data or target identities,
      mutable static payloads, leaked unpublished states, and immortal origins
      on unsupported layouts.
- [ ] Extend public MIR facades and dumps minimally; keep verifier structure
      split by declaration, instruction, ownership, and cleanup responsibility.

**Tests:** HIR-to-MIR order and lifecycle tests; exact MIR dumps; one-invariant
verifier mutations for identity, density, type, length, bytes, metadata,
descriptor fields, publication, owner state, and cleanup; determinism and
public-facade tests; repository gates.

**Exit criteria:** Verified MIR completely describes each literal descriptor
and immortal backing with no target offsets, dynamic-allocation ambiguity, or
unverified privacy bypass.

### STR3 — Emit deterministic immortal literal data on x86-64

**Purpose:** Realize verified static backing and generic immortal count
semantics without changing the public runtime boundary.

- [ ] Pool decoded byte sequences deterministically and emit one canonical
      empty backing plus collision-proof private symbols.
- [ ] Emit the exact shared `u8[]` header, metadata relocation, length, bytes,
      alignment, and immutable/relocation-read-only section placement.
- [ ] Materialize `Str` descriptors through existing class field layout and
      hidden-result/destination machinery with no allocator or byte-copy call.
- [ ] Change generated retain/release so a verified `u64::MAX` immortal count
      is a no-op and dynamic `u64::MAX - 1` retain terminates before collision.
- [ ] Derive legality from MIR static-allocation origin; arbitrary mutated MIR
      or ordinary dynamic handles cannot select immortal behavior.
- [ ] Reuse generic array metadata/finalizer tables and keep runtime ABI
      version 5 and the public C header unchanged.
- [ ] Keep pooling, data layout, strong-count lowering, and descriptor
      materialization in cohesive backend owners behind the x86-64 facade.

**Tests:** Layout/relocation/alignment tests for empty, ASCII, embedded zero,
`0x80`, and `0xff`; pooling and symbol determinism; assembly acceptance;
absence of allocation/copy calls; count sentinel/overflow/release tests;
malformed-MIR backend rejection; repeated literal native execution; runtime
header/version assertions; repository gates.

**Exit criteria:** Verified literals execute with immutable program-lifetime
backing, ordinary descriptor semantics, no per-use byte allocation/copy, and
no runtime ABI change.

### STR4 — Provide ordinary standard-library string behavior

**Purpose:** Demonstrate that the frozen compiler/library boundary supports
useful dynamic strings without compiler-selected method spellings.

- [ ] Add the canonical `std/str.ska` module with the exact public `Str`
      descriptor and safe ordinary initializer(s).
- [ ] Implement a representative public static factory that copies caller
      bytes into fresh shared storage.
- [ ] Implement length and checked byte observation as ordinary instance
      methods.
- [ ] Implement `O(1)` slicing through private instance/static helpers that
      copy an existing descriptor and adjust validated private bounds.
- [ ] Implement independent byte-array conversion and concatenation with fresh
      backing through ordinary array construction and slice assignment.
- [ ] Prove synthesized copy, assignment, destruction, shared retain/release,
      dynamic last-owner reclamation, arguments, results, and temporaries.
- [ ] Keep exact API names beyond the representative tested surface owned by
      the standard library; add no compiler lookup by those names.

**Tests:** Standard-library resolution/type-check tests; native goldens for
literal length/bytes, copies, reassignment, slicing/backing sharing, factory
copy isolation, conversion, concatenation, dynamic reclamation, embedded
zero/high bytes, and repeated evaluation; method-renaming tests proving no
compiler convention; repository gates.

**Exit criteria:** Canonical `Str` source provides the frozen representation
and representative dynamic behavior entirely through ordinary Skald methods
outside intrinsic literal materialization.

### STR5 — Harden, document, and promote strings as implemented

**Purpose:** Close the implementation with adversarial coverage, current
documentation, and no rollout-only structure.

- [ ] Complete every diagnostic and ownership matrix from the language and
      compiler contracts, including provider permutations and malformed MIR.
- [ ] Add independent-process phase/assembly/diagnostic determinism coverage
      for literals and canonical language-item loading.
- [ ] Audit touched lexer, syntax, module, resolution, HIR, MIR, verifier,
      backend, and standard-library owners by responsibility; resolve
      high-priority hotspots and index any lower-priority discoveries.
- [ ] Remove roadmap vocabulary and stale “not implemented” language from
      living code and documentation.
- [ ] Update grammar, language/compiler overviews, status, testing/debugging
      guidance, examples, and cross-links to the exact implemented boundary.
- [ ] Confirm runtime ABI/public header stability and artifact cleanliness.
- [ ] Complete and archive this roadmap after all focused and repository gates
      pass.

**Tests:** Focused documentation checker tests and link inspection; full
diagnostic, golden, determinism, robustness, public API, runtime, and compiler
suites; artifact-free `make check`, `make msrv-check`,
`make robustness-long`, and `git diff --check`.

**Exit criteria:** Strings are an implemented deterministic source-to-native
contract with frozen representation and literal semantics, all living
documentation is current, and the roadmap is ready to archive.

## Ordering and dependencies

STR0 combines literal parsing with language-item discovery because module
reachability must be known before resolution can assign the canonical class.
STR1 then freezes that identity into typed products before executable
representations exist. STR2 establishes explicit verified static ownership
before STR3 changes generated count behavior or emits immortal data. STR4
builds only on complete literal execution and demonstrates the intended
standard-library boundary. STR5 closes broad matrices and documentation after
all behavior is observable.

The roadmap depends on implemented private fields, private methods, public and
private static methods, exact-class lifecycle, shared primitive arrays, copied
slices and slice assignment, deterministic module providers, and verified
shared ownership. STR4 additionally needs ordinary checked `u64` range
arithmetic and conversion to the `i64` array-position type for its public
byte/range API. If those general operations are still absent after STR3, they
must land as a separate primitive-language prerequisite before STR4 rather
than as string-only intrinsics. They are not dependencies for STR0–STR3 or for
freezing the string representation. Loops, static fields, string operators,
Unicode, and a complete standard-library framework are not dependencies.
