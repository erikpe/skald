# Initial Module-System Implementation Roadmap

Status: in progress; MS5 is next.

This roadmap implements the frozen initial whole-program module system without
redefining it. The source-visible authority is
[Modules and Foreign Interoperation](../language/MODULES_AND_INTEROP.md#frozen-initial-module-system).
The provider, filesystem, entry, identity, loading, determinism, linkage, and
diagnostic authority is the
[Module-System Compiler Contract](../compiler/MODULE_SYSTEM.md). If
implementation discovers a choice that changes either contract, the affected
task stops until the authoritative document is revised explicitly.

The implementation extends the existing one-directional compiler pipeline:

```text
compilation request
  -> provider normalization and entry selection
  -> reachable parsed module graph
  -> whole-program resolution and type checking
  -> existing HIR, MIR, verification, backend, and artifact pipeline
```

The roadmap deliberately does not create a semantic linker or a second
lowering path. Resolution remains the only source-name selection phase. HIR
and lower phases continue to consume dense identities in one flat
whole-program representation.

## Current baseline

- The compiler exposes validated exact-case `ModulePath`, distinct
  request-local module/provider/package identities, module source-provenance
  records, and a typed driver `CompilationRequest`. These are foundational
  values only; no active pipeline or CLI entry consumes the request yet.
- `SourceDatabase` can own multiple source files, but the driver inserts one
  file and invokes every phase once.
- Syntax represents one `CompilationUnit` containing only top-level
  declarations. The lexer has no `::` token, and AST names do not preserve
  module-qualified source paths.
- Resolution collects one global top-level namespace, allocates declaration
  IDs in that source's declaration order, and selects any top-level `main` as
  the prospective entry.
- Type checking validates the selected `main`; HIR, MIR, verification, and the
  backend already carry one explicit entry-function identity through the host
  wrapper.
- Internal backend symbols are derived from dense semantic identities, so
  distinct same-named declarations need no path-based native mangling.
  External linkage currently stores the exact source symbol independently on
  each declaration.
- `compile_source_to_assembly` is a widely used repository-internal public
  convenience API. The real `skac` CLI accepts exactly one positional source,
  derives output from that file, and owns source I/O and artifact publication.
- Compiler unit tests own private phase behavior; compiler integration tests
  own public composition and process determinism; binary integration tests own
  the real CLI; and golden tests own complete rendered diagnostics and native
  behavior. The current golden runner treats every discovered `.ska` file as
  an independent entry and therefore needs an explicit multi-file fixture
  convention.

## Scope and implementation invariants

- Compile only the selected entry and the transitive closure of its imports.
  Do not scan, parse, or compile unrelated source files below roots.
- Treat anonymous ordinary roots, the active standard-library root, and an
  outside-root singleton entry as one unordered provider union. Configuration
  order never selects a winner.
- Keep `ModulePath`, `ModuleId`, `SourceId`, `ProviderId`, and `PackageId`
  distinct. Physical path identity, file contents, and path prefixes never
  substitute for logical module identity.
- Follow symlinks below roots, including escapes, while deriving module paths
  from the lexical root-relative spelling. Distinct logical paths remain
  distinct module instances even when they reach one physical source.
- Allocate final dense module and declaration identities in canonical
  module-path order, then source declaration order, then existing member
  order. Recursive discovery order, import order, root order, and entry choice
  must not leak into identities or output.
- Parse and retain imports as source declarations, but keep graph reachability
  separate from import binding. Loading resolves canonical import sources;
  resolution later decides what local names they bind.
- Keep top-level declarations private by default. `public` controls only
  cross-module Skald lookup and never changes member access or native export.
- Keep module bindings and ordinary declaration bindings separate. Resolve
  qualified and unqualified source forms through one centralized,
  current-module-aware service.
- Reject every cycle, including self-import, with the complete ordered cycle.
  There is no module initialization or initialization ordering.
- Preserve separate source declaration identities for compatible external
  declarations while assigning them one compilation-wide external-link
  identity. Reject incompatible declarations before backend emission.
- Preserve the existing one-file convenience API as an in-memory singleton
  adapter. It must not become a second compiler pipeline or expose extra
  source-language semantics.
- Supply standard-library provider plumbing and an installation-owned default
  root, but do not design or add any Skald standard-library declarations in
  this roadmap.
- Keep all filesystem and installation inputs explicit at the driver/loading
  boundary so unit tests do not depend on ambient working directory, default
  standard-library layout, or host directory enumeration order.
- Keep new Rust implementation behind responsibility-oriented facades and
  focused submodules. Do not grow `mod.rs`, the driver CLI parser, or the
  resolver orchestration file into the module-system implementation.
- Relative imports, wildcards, re-exports, package-private access, manifests,
  versions, directory modules, separate compilation, binary libraries,
  top-level state, and native export remain excluded.

## Niflheim audit implications

Niflheim provides useful evidence for multi-module staging but is not an
implementation template:

- Its loader proves the value of a program object keyed by logical module path
  and of building every module's declaration context before checking bodies.
  Skald should retain those two-pass properties.
- Niflheim resolves from one project root. Skald must instead put candidate
  lookup behind the provider union from the start; adding provider precedence
  or root bindings temporarily would create the wrong source model.
- Niflheim's resolver combines recursive loading, import surfaces,
  flattening/re-export behavior, visibility, and qualified lookup. Those
  policies spread into later type lookup. Skald's smaller frozen semantics
  permit three explicit owners: graph loading, per-module declaration
  indexing, and import-aware use resolution.
- Niflheim supports exported imports, flattened imports, and multi-component
  bindings. Skald must not carry representation or algorithms for those
  excluded forms merely because the sister project has them.
- Niflheim can use path-and-name semantic identities. Skald already relies on
  dense typed IDs throughout HIR, MIR, verification, and code generation, so
  module ownership augments those IDs instead of replacing them.
- A recursive `visiting` set is sufficient to notice a cycle but not to meet
  Skald's diagnostic contract. Skald needs an ordered DFS stack or equivalent
  predecessor data that reconstructs and labels the complete cycle.

## Progress

- [x] MS0 — Establish module identities and request contracts
- [x] MS1 — Parse imports, visibility, and qualified source paths
- [x] MS2 — Normalize providers and resolve filesystem candidates
- [x] MS3 — Select the entry and load the reachable module graph
- [x] MS4 — Carry module ownership through whole-program IR
- [ ] MS5 — Collect and resolve a deterministic multi-module program
- [ ] MS6 — Resolve module imports and qualified uses
- [ ] MS7 — Resolve selective imports and ordinary bindings
- [ ] MS8 — Coalesce compatible external ABI declarations
- [ ] MS9 — Integrate compilation requests with the driver and CLI
- [ ] MS10 — Harden determinism, diagnostics, fixtures, and documentation

## PR-sized implementation sequence

### MS0 — Establish module identities and request contracts

**Purpose:** Introduce stable vocabulary and ownership boundaries before
filesystem, syntax, or resolver consumers depend on ad hoc strings and paths.

- [x] Add a validated, non-empty, exact-case `ModulePath` value with canonical
      `::` rendering, component iteration, ordering, and parsing from logical
      CLI text and validated filesystem components. Reuse Skald's identifier
      policy rather than inventing a path-only identifier grammar.
- [x] Add dense request-local `ModuleId`, `ProviderId`, and `PackageId`
      identities beside the existing semantic IDs. Keep `SourceId` in the
      source owner and make conversions between these identities impossible by
      type.
- [x] Define module provenance records that can retain canonical logical path,
      module/source/provider/package identities, lexical root-relative path,
      deterministic display source path, and optional canonical I/O target
      without treating the latter as semantic identity.
- [x] Define the internal compilation-input model: positional file or logical
      entry, repeatable anonymous roots, default/replacement/disabled standard
      library, target, and the driver-owned artifact options that affect output
      selection.
- [x] Capture process-dependent inputs such as working directory and installed
      default standard-library root at one request-construction boundary.
      Provider and graph code should receive explicit values.
- [x] Reserve a narrow `module` facade for path, provider, and graph concepts,
      with implementation split by responsibility under it.
- [x] Keep the current `compile_source_to_assembly` signature available and
      document it as a compatibility convenience to be adapted in MS9.
- [x] Add exact unit tests for valid/invalid components, empty paths, `::`
      parsing/rendering, ordering, identity type separation, and request-option
      conflicts that do not require filesystem access.
- [x] Update compiler phase/API documentation only for the new internal
      contracts; do not claim source or CLI support.

**Tests:** Focused module-path, identity, request-model, and public-path tests,
then `make check` and `make msrv-check`.

**Exit criteria:** Later tasks can exchange typed module and request data
without using display strings, canonical files, or option positions as
semantic identity, while all current one-file behavior remains unchanged.

### MS1 — Parse imports, visibility, and qualified source paths

**Purpose:** Produce complete source-shaped syntax for every frozen module form
before graph loading or semantic lookup consumes it.

- [x] Add one lexer token for `::` with exact token/span/dump behavior. Keep
      `import`, `from`, `as`, and `public` contextual rather than reserving
      them globally.
- [x] Add AST forms for module imports, one-identifier module aliases,
      selective imports, selective declaration aliases, and declaration
      visibility. Preserve introducer, component, alias, separator, list, and
      full-declaration spans needed by later diagnostics.
- [x] Represent a qualified declaration spelling as an unresolved component
      chain wherever a top-level declaration name may currently appear:
      types, calls, construction, inheritance, interface claims, casts, type
      tests, and other existing declaration-bearing source positions.
      Parsing must not guess where a module binding ends and the declaration
      leaf begins.
- [x] Parse all imports before top-level declarations, exact `::` module paths,
      optional one-identifier module aliases, and comma-separated selective
      lists without trailing commas.
- [x] Parse optional `public` on functions, external declarations, classes,
      and interfaces only. Preserve private-by-default behavior in the AST.
- [x] Reject wildcard imports, relative/parent spellings, empty components,
      multi-segment aliases, misplaced imports, trailing selective commas, and
      malformed qualified uses with focused recovery that reaches later
      declarations.
- [x] Ensure contextual words remain valid identifiers outside their exact
      import and visibility positions.
- [x] Extend token and AST dumps deterministically. Ensure a one-file AST with
      no imports retains a compact, stable representation or receives one
      intentional expectation update.
- [x] Give the still-single-file semantic adapter a structured unsupported-
      module diagnostic for a parsed import rather than ignoring it or
      panicking. Full import semantics arrive in MS3 through MS7.
- [x] Update the implemented grammar only when a form becomes accepted by the
      complete compiler; until then keep the frozen grammar as the future
      authority and describe syntax support as phase-local.

**Tests:** Focused lexer, parser, contextual-word, AST dump, nesting, malformed
input, and recovery tests across every qualified-use context, followed by
`make check`, `make msrv-check`, and `make robustness-long`.

**Exit criteria:** The frontend losslessly represents every frozen import,
visibility, and qualified-use form, rejects every excluded spelling at syntax
ownership, and never performs module lookup.

**Implementation record (2026-07-27):** Complete. Qualified spellings use a
compact tagged name representation: ordinary names retain their previous AST
footprint, while qualified names own normalized text plus component and
separator spans. This preserves the existing recursive syntax budget while
allowing iterative paths with thousands of components. The single-file
resolver reports `RES023` for imports and qualified uses. Focused lexer,
parser, dump, recovery, contextual-word, long-path, and resolver-adapter tests
pass, as do `make check`, `make msrv-check`, and `make robustness-long`.

### MS2 — Normalize providers and resolve filesystem candidates

**Purpose:** Implement the unordered provider union and filesystem rules as a
testable owner independent of recursive loading and CLI parsing.

- [x] Normalize each configured root against the captured working directory,
      canonicalize the root itself, require an existing directory, and retain
      both canonical equivalence data and configuration provenance.
- [x] Coalesce every spelling of one canonical directory into one provider,
      including ordinary-root/standard-library duplication, and choose
      deterministic display provenance independent of option order.
- [x] Assign request-local provider and package provenance without deriving
      either from logical path prefixes. Treat coalesced configurations as one
      provider.
- [x] Map a logical `a::b` candidate to lexical `<root>/a/b.ska` without
      scanning the whole root. Validate the canonical `.ska` suffix and exact
      identifier spelling of each path component.
- [x] Verify exact component case even on case-insensitive hosts. Diagnose
      host collisions that cannot be distinguished deterministically rather
      than selecting an enumeration winner.
- [x] Follow file and directory symlinks below a root, including targets
      outside it. Retain lexical module identity while diagnosing reached
      broken/cyclic links, unreadable paths, non-regular files, and invalid
      components.
- [x] Resolve each exact logical path to missing, unique, or ambiguous.
      Ambiguity must report every distinct normalized provider regardless of
      equal bytes, hard links, or common canonical targets.
- [x] Prove that partial tree overlap succeeds when exact module paths differ,
      and that one physical file reached through two logical paths yields two
      candidate records rather than semantic deduplication.
- [x] Keep candidate canonical targets confined to I/O, optional byte caching,
      and diagnostics; provider and module ambiguity keys use provider plus
      logical path.
- [x] Use injectable filesystem-facing helpers where needed to test
      deterministic case and enumeration policy without weakening real-host
      integration tests.

**Tests:** Focused temporary-filesystem tests for equivalent roots, standard
root coalescing, partial overlap, exact collisions, hard links, symlink files
and directories, symlink escapes, broken/cyclic links, non-files, unreadable
candidates, case mismatches, and deterministic candidate ordering, followed
by `make check` and `make msrv-check`.

**Exit criteria:** Given normalized configuration and one logical path, the
provider layer returns the one contract-defined outcome with stable
provenance and diagnostics, without loading or selecting declarations.

**Implementation record (2026-07-27):** Complete. The `module` facade now
owns normalized provider configurations, deterministic canonical-root
coalescing and request-local provider/package allocation, retained lexical
and display provenance, and exact component-by-component candidate probing.
Lookup returns missing, unique, ambiguous, or structured reached-filesystem
failures without reading source contents or scanning provider trees.
Canonical descendant targets are retained only for I/O and diagnostics;
provider plus logical path remains the candidate identity. Focused
temporary-filesystem tests cover empty unions, equivalent and standard roots,
partial overlap, ordering, case policy, hard links, file and directory
symlinks including escapes, common targets, broken and cyclic links,
non-files, and unreadable paths. `make check` and `make msrv-check` pass.

### MS3 — Select the entry and load the reachable module graph

**Purpose:** Turn an entry selector and provider union into one deterministic,
parsed, acyclic whole-program graph.

- [x] Implement logical entry selection through ordinary provider lookup and
      positional entry validation for existence, regular-file status,
      readability, canonical suffix, and valid stem/components.
- [x] Determine positional containment lexically against each provider's
      canonical root and retained normalized spellings. Reject multiple
      provider identities; do not canonicalize descendant targets to infer
      containment.
- [x] Create an outside-root singleton provider that exposes exactly the
      selected file under its top-level stem. Make it participate in ordinary
      ambiguity and cycle rules without exposing its parent directory.
- [x] Intern rooted file selection and logical selection of the same module as
      one graph node and one eventual `ModuleId`.
- [x] Load the selected source, lex and parse it, resolve every canonical
      import source through MS2, and continue to graph closure. Multiple local
      bindings or selective items from one source module add only one graph
      edge and never reload the source.
- [x] Keep source acquisition, graph reachability, and import source spans in
      the loader; do not construct module aliases, selective declaration
      bindings, or public surfaces here.
- [x] Stage recursive discovery separately from final dense identity
      allocation. Finalize modules and their source instances in canonical
      module-path order so online DFS/work-queue order cannot leak through
      `ModuleId`, `SourceId`, diagnostics, or later declaration allocation.
- [x] Preserve distinct `SourceId`, AST, and graph nodes when distinct logical
      paths reach one physical source. An optional shared byte cache must sit
      below source-instance creation.
- [x] Reject self-imports and longer cycles with the complete canonical module
      chain and the corresponding import spans in edge order.
- [x] Propagate missing/ambiguous candidates and unreadable or malformed
      imported files as structured, cross-file compilation diagnostics. Stop
      erroneous phase products before semantic resolution.
- [x] Expose a deterministic graph dump or equivalent test view containing
      entry, module path/provenance, source identity, direct imports, and
      canonical graph order.

**Tests:** Focused entry-selection, singleton, rooted/logical equivalence,
reachability, unused-import, duplicate-edge, physical-source/logical-instance,
missing/ambiguous import, malformed imported source, self-cycle, longer-cycle,
graph dump, and allocation-order tests, followed by `make check` and
`make msrv-check`.

**Exit criteria:** A valid request produces exactly one parsed graph containing
the selected entry and reachable imports in deterministic order, or one
structured loading failure satisfying the frozen diagnostic contract.

**Implementation record (2026-07-27):** Complete. The `module` facade now
selects logical and positional entries, derives rooted identities by lexical
containment, creates isolated outside-root singleton providers, and loads only
the reachable import closure. Discovery caches source text and canonical
import sources; finalization inserts sources, reparses the retained ASTs, and
allocates `SourceId` and `ModuleId` in canonical module-path order. Repeated
bindings share one direct edge while retaining every import span, and distinct
logical paths to one physical target retain separate source, AST, provenance,
and graph instances. Missing, ambiguous, invalid, unreadable, non-UTF-8, and
malformed reachable sources produce structured cross-file diagnostics.
Iterative deterministic cycle detection reports complete self and longer
cycles without recursive stack growth. The graph facade exposes structural
inspection plus an exact deterministic dump. Fifteen focused entry, loading,
identity, diagnostic, symlink, singleton, cycle, and dump tests pass, including
a 10,000-node cycle-walk regression, as do `make check` and
`make msrv-check`.

### MS4 — Carry module ownership through whole-program IR

**Purpose:** Make existing whole-program representations capable of preserving
module provenance before the resolver starts selecting across modules.

- [x] Add selected `ModuleId` and a dense module metadata table to resolved
      program state. Give every top-level resolved declaration explicit owning
      `ModuleId`; members continue to derive class/interface ownership while
      their enclosing declaration supplies module ownership.
- [x] Propagate the selected entry module and declaration ownership through
      checked HIR and MIR wherever needed to retain the frozen identity and
      diagnostic contract. Do not introduce source-name lookup below
      resolution.
- [x] Keep flat existing declaration and definition tables. Module metadata
      indexes those tables; it does not create per-module HIR/MIR pipelines or
      a semantic linking phase.
- [x] Extend table constructors and verifiers to reject unknown module owners,
      mismatched declaration ownership, duplicate module paths, invalid
      selected-entry ownership, and non-dense module metadata.
- [x] Update resolved, HIR, and MIR dumps with stable module ownership while
      keeping backend labels based on collision-free semantic identities.
- [x] Adapt existing single-source resolution/test helpers to synthesize one
      request-local module and preserve all existing tests and public phase
      composition.
- [x] Prove that changing only selected-entry metadata cannot renumber module
      or declaration identities when the reachable module set is unchanged.
- [x] Update phase/IR documentation for module ownership without claiming
      multi-file source acceptance.

**Tests:** Focused resolved/HIR/MIR representation, dump, table-density,
ownership-mutation, entry-metadata, backend-symbol, public API, and existing
single-file regression tests, followed by `make check` and `make msrv-check`.

**Exit criteria:** Every whole-program phase can retain and verify module
ownership while the compiler still uses one flat semantic program and the
existing single-source adapter remains behaviorally compatible.

**Implemented:** `ProgramModuleTable` now owns validated dense
`ModuleProvenance` entries and selected-entry metadata, rejects non-dense
identities, duplicate logical paths, and unknown selected modules, and can be
copied directly from a loaded graph. Resolved function, class, and interface
declarations carry explicit module owners; type checking and MIR lowering
preserve both those owners and the unchanged module table while retaining the
existing flat declaration/definition layout. The one-AST resolver synthesizes
one request-local `main` module over the AST source, so the public single-file
pipeline remains one ordinary semantic path without making sibling files
searchable. Resolved, HIR, and MIR dumps now expose selected module, dense
module provenance, and top-level owners. MIR verification reports malformed
module metadata, unknown owners, and entry-function/selected-module mismatch.
Focused constructor, selected-entry determinism, cross-phase propagation,
verifier mutation, dump, public-API, backend, and existing regression coverage
passes, as do `make check` and `make msrv-check`.

### MS5 — Collect and resolve a deterministic multi-module program

**Purpose:** Replace the one-global-namespace assumption with per-module
declaration indexing and deterministic whole-program identity allocation.

- [ ] Add a resolver entry point over the parsed module graph while retaining
      the current one-AST facade as a synthesized singleton adapter.
- [ ] Separate program work into declaration collection, hierarchy/signature
      resolution, and body resolution so every reachable module's declarations
      exist before any body selects a declaration.
- [ ] Allocate functions, classes, interfaces, and existing member identities
      in canonical module-path order, then source declaration/member order,
      independent of graph discovery, root configuration, import spelling, or
      selected entry.
- [ ] Build one ordinary top-level declaration table per module. Reject
      duplicate leaf names only within that module; same leaf names in
      different modules receive distinct identities.
- [ ] Record private-by-default/public visibility and build a direct public
      surface containing only the module's own supported public declarations.
      Do not add imports, flattening, or re-exports to that surface.
- [ ] Resolve every module's unqualified local declaration uses against its
      own table, preserving existing lexical binding and member lookup rules.
      Imported lookup remains delegated to MS6 and MS7.
- [ ] Select `main` only from the selected entry module. Allow private entry
      `main`; treat every `main` elsewhere as ordinary; preserve the existing
      exact type-check signature and host-wrapper checks.
- [ ] Lower and verify one flat HIR/MIR program containing all reachable
      definitions, including modules imported only for reachability.
- [ ] Add deterministic cross-file duplicate, hierarchy, signature, entry, and
      local-body diagnostics with source ownership intact.

**Tests:** Focused multi-module collection, same-leaf distinct identity,
private/public surface, forward cross-module staging, canonical allocation,
selected versus non-selected `main`, flat HIR/MIR dump, verifier, backend
symbol, and local-only body tests, followed by `make check` and
`make msrv-check`.

**Exit criteria:** The semantic pipeline compiles a reachable graph whose
modules use only their own declarations, with deterministic global IDs and
exactly one selected entry, without yet granting imported names.

### MS6 — Resolve module imports and qualified uses

**Purpose:** Implement direct module bindings and central qualified declaration
selection across every existing declaration-use context.

- [ ] Build each module's module-binding namespace from direct module imports.
      The default binding is the complete canonical module path; an alias is
      exactly one local identifier.
- [ ] Permit multiple distinct local bindings of one canonical module while
      retaining one graph node and dependency edge. Reject repetition or
      conflict only under the frozen local-binding rules.
- [ ] Keep module bindings in the `::`-selected namespace so aliases may share
      spelling with a top-level declaration, parameter, or lexical local
      without contaminating ordinary lookup.
- [ ] Add one centralized current-module lookup service that splits an
      unresolved qualified chain through a directly imported module binding,
      then selects exactly one declaration leaf from the target module.
- [ ] Resolve qualified functions, external functions, classes, and interfaces
      in every source context represented by MS1, returning existing dense
      declaration identities rather than path/name pairs.
- [ ] Enforce direct import and target visibility. Knowing an absolute logical
      path, importing an ancestor, or importing a module that imports the
      target must not grant access.
- [ ] Diagnose unknown bindings, partial paths, qualified private declarations,
      missing leaves, wrong declaration kinds, descendant assumptions, and
      attempted transitive access with importing-use and target labels.
- [ ] Ensure aliases affect only the local source spelling in diagnostics;
      canonical module/declaration ownership remains visible and unchanged.
- [ ] Update resolved dumps to show selected identities and canonical owners,
      not unresolved paths. Keep HIR and lower phases path-free.

**Tests:** Focused default/alias/multiple-binding, binding-conflict,
module/ordinary namespace overlap, direct-import, privacy, transitive and
descendant rejection, wrong-kind, missing-leaf, all qualified-use contexts,
resolved dump, type-check, HIR/MIR, and native call tests, followed by
`make check` and `make msrv-check`.

**Exit criteria:** Every qualified use either resolves once through a direct
module binding to an existing declaration ID or receives one deterministic
module-aware diagnostic; no later phase repeats the lookup.

### MS7 — Resolve selective imports and ordinary bindings

**Purpose:** Complete the ergonomic unqualified import form without implicit
flattening, wildcard behavior, or re-export surfaces.

- [ ] Resolve each selective item from its canonical import-source module,
      never through an earlier alias or source-order-dependent binding.
- [ ] Permit only public top-level classes, interfaces, defined functions, and
      external functions owned directly by the target module. Reject private,
      missing, wrong-kind, member, module-binding, and merely imported items.
- [ ] Introduce the source name or one explicit alias into the importing
      module's ordinary top-level namespace while retaining the target's
      original identity and canonical owner.
- [ ] Permit multiple local names for one canonical declaration. Reject a
      repeated local ordinary name even when it would select the same target,
      and reject collisions with declarations owned by the importing module.
- [ ] Preserve existing lexical shadowing: parameters and nested locals may
      shadow a selectively imported ordinary name under the same rules as a
      module-local top-level declaration.
- [ ] Keep selective reachability independent from binding count and do not
      bind the source module implicitly. Requiring both qualified and
      unqualified access still requires both import declarations.
- [ ] Keep target public surfaces direct and immutable; selective imports do
      not alter downstream visibility or create re-exports.
- [ ] Produce cross-file labels for collisions, privacy, missing targets, and
      wrong declaration kinds, and retain canonical target ownership in dumps.

**Tests:** Focused unaliased/aliased/multiple-name imports, local declaration
collisions, repeated binding, lexical shadowing, private/missing/wrong-kind
targets, alias-source independence, no implicit module binding, no re-export,
resolved/HIR/MIR dumps, type-check, and native tests, followed by `make check`
and `make msrv-check`.

**Exit criteria:** Selective imports add exactly the requested public
declaration identities to ordinary lookup, with all frozen collision,
shadowing, reachability, and non-re-export rules enforced centrally.

### MS8 — Coalesce compatible external ABI declarations

**Purpose:** Give repeated cross-module foreign declarations an explicit
compilation-wide linkage identity and reject incompatible trusted assertions
before backend emission.

- [ ] Add a dense `ExternalLinkId` and compilation-wide external-link table
      distinct from source `FunctionId`. Each source declaration retains its
      module ownership, visibility, source name, and function identity.
- [ ] Group valid external declarations across distinct modules by exact
      foreign symbol only after their source signatures have resolved to
      canonical ABI-relevant types.
- [ ] Coalesce declarations only when calling convention, ordered parameter
      types/count, and result type are identical. Ignore parameter names,
      aliases, visibility, and module ownership for ABI compatibility.
- [ ] Diagnose every incompatible declaration for one foreign symbol in one
      deterministic report, label all declaration sites, and describe the
      signature differences before HIR/backend emission.
- [ ] Preserve ordinary same-module duplicate-name rejection and keep Skald
      function definitions entirely outside external coalescing.
- [ ] Carry `ExternalLinkId` through resolved linkage, HIR, MIR, verification,
      dumps, and backend call selection. Resolve the exact native symbol once
      from the external-link table.
- [ ] Verify that every external declaration references a valid compatible
      link entry and that internal definitions never do.
- [ ] Prove root/import/declaration discovery permutations do not change link
      IDs, diagnostics, assembly, or calls.

**Tests:** Focused identical and incompatible cross-module signatures,
parameter-name differences, primitive/result differences, same-module
duplicates, internal/external separation, link-table density, verifier
mutations, resolved/HIR/MIR dumps, backend symbol/call tests, and native
linkage tests, followed by `make check` and `make msrv-check`.

**Exit criteria:** Compatible source declarations remain distinct semantic
functions but share one verified external-link identity, while every
incompatible symbol group fails deterministically before code generation.

### MS9 — Integrate compilation requests with the driver and CLI

**Purpose:** Expose the completed module pipeline through the supported
library/driver boundary and exact frozen command-line behavior.

- [ ] Add one request-based source-to-assembly orchestration API that owns
      provider normalization, graph loading, multi-module semantics, and the
      existing backend pipeline. Keep source I/O in loading/driver owners and
      artifact publication in the driver.
- [ ] Reimplement `compile_source_to_assembly` as an in-memory singleton
      request adapter so existing phase/public tests use the same semantic
      pipeline without gaining filesystem-root discovery.
- [ ] Extend CLI parsing with mutually exclusive positional file and
      `--entry`, repeatable `--module-root`, one optional `--stdlib-root`, and
      `--no-stdlib`. Preserve existing target, emit, output, help, and version
      behavior.
- [ ] Supply the installed default standard-library root through one
      test-injectable driver/toolchain configuration. A replacement fully
      replaces it; disabling removes it; neither form adds lookup precedence
      or eager loading.
- [ ] Classify missing/repeated/conflicting selectors and options as usage
      errors; classify reached source/module failures as compilation errors;
      and preserve established I/O/toolchain/artifact exit boundaries.
- [ ] Implement positional output defaults from the input path and logical
      entry defaults from the final module component in the current directory.
      Preserve explicit `-o`/`--output`.
- [ ] Apply the frozen existing input/output-alias protection to the selected
      positional source and preserve recoverable pending-artifact publication.
      Do not silently broaden alias policy based on imports or physical module
      deduplication.
- [ ] Update help text and driver documentation with both entry forms, all
      module/standard-library options, and output examples.
- [ ] Cover option-order independence, non-UTF-8 OS arguments where relevant,
      paths with spaces, relative working directories, and standard-library
      installation injection through the real binary.

**Tests:** Focused request-pipeline and driver unit tests, public API tests,
real `skac` CLI tests for every selector/option/output/status combination, and
artifact/toolchain regressions, followed by `make check` and
`make msrv-check`.

**Exit criteria:** Both frozen entry forms compile through one request-based
whole-program pipeline with exact root, standard-library, output, diagnostic,
and process behavior, while the old source-text convenience API remains a
thin adapter.

### MS10 — Harden determinism, diagnostics, fixtures, and documentation

**Purpose:** Close the roadmap with complete cross-boundary evidence and
promote the frozen design to implemented status only after that evidence
passes.

- [ ] Define and document a golden-fixture convention that groups one entry,
      supporting module trees, entry mode, and root/standard-library arguments
      without causing support `.ska` files to be discovered as independent
      cases. Preserve every existing one-file fixture unchanged.
- [ ] Add end-to-end success cases for split logical trees, qualified and
      selective imports, multiple bindings, private entry `main`, non-entry
      `main` functions, replacement/disabled standard library, and compatible
      external declarations.
- [ ] Add exact compile-failure goldens for the complete required diagnostic
      list in the compiler contract, using multi-file labels where ownership,
      ambiguity, cycles, privacy, or ABI conflicts require them.
- [ ] Add filesystem integration matrices for equivalent roots, same logical
      path from distinct providers regardless of contents/physical target,
      symlink escapes, distinct logical instances of one physical file, exact
      case, singleton visibility, and positional containment aliases.
- [ ] Extend cross-process determinism coverage to permute root option order,
      import declaration order, discovery shape, equivalent root spellings,
      and selected entry with an unchanged reachable set. Compare graph,
      resolved, HIR, MIR, diagnostics, assembly, and native observations as
      applicable.
- [ ] Prove unrelated root files are not read or compiled and that malformed
      unreachable files cannot affect a build.
- [ ] Add hostile frontend/import-path corpus cases and retain the smallest
      focused regression for any failure found.
- [ ] Remove transitional unsupported-module diagnostics/adapters that are no
      longer needed, dead one-file orchestration, and duplicated lookup or
      path-normalization helpers.
- [ ] Update the implemented grammar, language/compiler status matrix,
      compiler phase and driver documents, testing guide, examples, and module
      contracts' implementation-status wording. Keep the frozen semantics
      unchanged.
- [ ] Review every deferred feature boundary and ensure no manifest,
      re-export, wildcard, package-private, directory-module, separate-
      compilation, module-initialization, or native-export behavior slipped
      into the implementation.

**Tests:** All focused module/provider/resolution/type-check/HIR/MIR/verifier/
backend/driver suites, public API and process-determinism tests, real CLI and
multi-file goldens, then `make check`, `make msrv-check`, and
`make robustness-long`.

**Exit criteria:** Every frozen positive rule and required diagnostic has an
owning test, identities and artifacts are stable across independent process
and ordering permutations, living documentation describes the implemented
compiler, and no transitional module path remains.

## Ordering and dependencies

| Task | Depends on | Stable boundary produced |
|---|---|---|
| MS0 | Frozen language/compiler contracts | Typed module identities and request vocabulary |
| MS1 | MS0 path vocabulary | Complete source-shaped module syntax |
| MS2 | MS0 provider/request vocabulary | Deterministic provider candidate resolution |
| MS3 | MS1, MS2 | Parsed reachable acyclic module graph |
| MS4 | MS0 | Module-owned flat whole-program IR |
| MS5 | MS3, MS4 | Deterministic multi-module declaration program |
| MS6 | MS5 | Direct module bindings and qualified lookup |
| MS7 | MS5, MS6 lookup service | Selective ordinary-name lookup |
| MS8 | MS5, MS7 | Compilation-wide external-link identities |
| MS9 | MS3–MS8 | Supported request pipeline and CLI |
| MS10 | MS9 | Complete evidence and implemented documentation |

MS1 and MS2 can be developed independently after MS0, and MS4 can begin once
MS0 is stable. Merge order remains the numbered order so every main-branch
state keeps the ordinary validation gate green and no consumer lands before
its representation or contract.

## Quality gates and completion policy

Every task runs its narrow owning tests while developing and finishes with
`make check`. Every Rust, manifest, or accepted-syntax task also runs
`make msrv-check`. MS1 and MS10 run `make robustness-long`; any minimized
regression becomes part of the ordinary suite. Filesystem tests use temporary
directories and do not depend on machine-global roots or an installed SDK.

No task marks the module system implemented merely because one entry form or
one import spelling works. The status matrix and implemented grammar change
only in MS10 after the complete source, filesystem, graph, identity, linkage,
CLI, diagnostic, determinism, and end-to-end obligations pass.

When MS10 is complete, mark every progress item complete, update the roadmap
index, change this status to complete, and archive the roadmap under
`docs/archive/` according to the repository roadmap process.
