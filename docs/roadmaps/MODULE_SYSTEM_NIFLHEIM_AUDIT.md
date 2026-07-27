# Niflheim Module-System Audit

Status: audit complete against Niflheim commit
`3dcd543620bfdc14c0b7c70a09364960e28174c9`; the resulting Skald direction is
developed in the
[initial module-system proposal](SKALD_INITIAL_MODULE_SYSTEM_PROPOSAL.md).

This document audits Niflheim's implemented module and import system as design
input for Skald. It describes observed behavior, architecture, strengths,
limitations, and consequences. It does not freeze Skald syntax or semantics;
the later Skald proposal incorporates additional root and entry decisions that
supersede the preliminary recommendations here. The current Skald contract
remains the single-file behavior in
[Modules and Foreign Interoperation](../language/MODULES_AND_INTEROP.md), and
feature maturity remains owned by the
[status matrix](../language/STATUS.md).

## Executive conclusion

Niflheim demonstrates that a useful whole-program module system can be built
around a small set of durable ideas:

- one canonical identity for each loaded module and top-level declaration;
- imports as the only route to cross-module visibility;
- declarations private by default and explicitly exported;
- local-first lookup with explicit qualification for disambiguation;
- an entry module distinct from the host ABI `main` symbol; and
- deterministic whole-program lowering after the reachable import graph is
  known.

Those ideas fit Skald well. Niflheim's complete surface and implementation
should not be copied wholesale. Its single source root conflates project,
standard-library, and dependency namespaces. Its aliases still make every
imported export eligible for unqualified lookup. Root flattening and
multi-segment aliases require substantial namespace-merging machinery.
Resolution owns file I/O, lexing, parsing, graph construction, and partial
visibility checking, while later layers repeat related lookup rules. Its
native mangling is not injective. These are material costs for Skald, whose
existing source database, structured diagnostics, typed dense identities, and
phase boundaries provide a cleaner starting point.

The smallest coherent first Skald design should therefore be narrower than
Niflheim:

- whole-program compilation of the reachable graph;
- one module per file, with no source `module` declaration initially;
- private-by-default top-level declarations and one public marker;
- imports that bind modules, with optional explicit short aliases;
- qualified cross-module use only in the first version;
- no re-exports, root flattening, selective symbol imports, or separate
  compilation initially;
- an explicit distinction between package identity, logical module path, and
  physical source location; and
- compilation-wide dense declaration IDs, with module ownership retained as
  metadata rather than encoded into the ID.

This subset provides standard- and external-library access and useful codebase
visibility without committing Skald to Niflheim's most complex namespace
features.

## Audit basis

The audit inspected Niflheim's tracked implementation and documentation at the
commit named above, especially:

- `compiler/resolver.py`;
- `compiler/frontend/ast_nodes.py`,
  `compiler/frontend/declaration_parser.py`, and
  `compiler/grammar/niflheim_v0_1.ebnf`;
- `compiler/typecheck/module_lookup.py` and the cross-module type-check
  helpers;
- `compiler/semantic/symbols.py`, `compiler/semantic/ir.py`,
  `compiler/semantic/lowering/`, and `compiler/semantic/linker.py`;
- `compiler/backend/program/symbols.py`,
  `compiler/backend/lowering/program.py`, and both native target emitters;
- `docs/LANGUAGE_MVP_SPEC_V0.1.md`,
  `docs/GRAMMAR_EBNF.md`, and
  `docs/archive/PROPER_MODULE_SEMANTICS_PLAN.md`;
- the standard-library modules under `std/`; and
- resolver, cross-module type-check, linker, identity, and CLI integration
  tests.

The following focused suites passed during this audit:

- 24 resolver tests;
- 55 cross-module type-check tests;
- 15 semantic linker and symbol-index tests; and
- 7 multi-module CLI/codegen tests.

The tests strongly establish the common successful paths and namespace
conflicts. Gaps identified below are based on direct implementation inspection
or focused probes and are labeled accordingly.

## Implemented model at a glance

| Concern | Niflheim behavior |
|---|---|
| Compilation unit | One `.nif` file is one module. |
| Module declaration | None; identity comes from the file path. |
| Canonical module path | Tuple of path segments relative to one project root. |
| Module lookup | `a.b` maps to `<project-root>/a/b.nif`. |
| Program boundary | Entry module plus all recursively reachable imports. |
| Compilation mode | Whole-program only. |
| Import cycles | Rejected during depth-first graph loading. |
| Top-level namespace | Classes, interfaces, functions, and external functions share one namespace per module. |
| Default visibility | Top-level declarations are private to their defining module. |
| Public visibility | `export` exposes a declaration to importers. |
| Qualification | Full imported bind path or explicit import alias. |
| Unqualified lookup | Local declaration first, otherwise one unique exported match from direct imports. |
| Re-export | Supported for bound modules and flattened exported surfaces. |
| Entry point | `fn main() -> i64` in the CLI-selected entry module only. |
| Canonical declaration identity | Module path plus declaration/member name, plus constructor ordinal where needed. |
| Native entry | A generated or aliased ABI-visible `main` selects the canonical entry function. |
| Standard library | Ordinary `std.*` source modules found through the same project root. |
| External libraries | No package or dependency model; sources must be reachable under the one root. |
| Separate compilation | Not supported. |

## Source and physical module identity

### Path-derived identity

`resolve_program(entry_file, project_root)` canonicalizes the entry path and
root. If no root is supplied, the entry file's parent is the root. The entry
module is the entry file's path relative to that root with `.nif` removed:

```text
<root>/app/main.nif  ->  app.main
<root>/util/math.nif ->  util.math
```

An import reverses the same mapping:

```text
import util.math; -> <root>/util/math.nif
```

There is no declaration inside the source that can disagree with the path.
This avoids declared-versus-physical identity conflicts and makes resolution
predictable. It also means moving a file changes every canonical class,
interface, function, member, metadata, and emitted symbol identity owned by
that module.

The module grammar accepts identifier segments only. There are no relative
imports, parent traversal, file extensions, string paths, or directory-module
files. Filesystem case behavior therefore leaks into module availability, even
though the logical grammar itself does not describe case-normalization rules.

### One root for every source kind

The resolver has one root, not a search path or package map. Niflheim's build
script passes the repository root as `--project-root`; this is what makes both
`samples.*` and `std.*` imports work. Isolated integration tests copy required
standard-library files into a temporary project's `std/` directory.

Consequences:

- `std` has no privileged compiler meaning;
- a project can shadow or replace `std.*` merely by controlling files under
  its selected root;
- a third-party library must be copied, vendored, or otherwise made visible
  below that root;
- two dependencies cannot independently own the same logical module path; and
- there is no version, package, dependency-alias, lockfile, or provenance
  identity available to diagnostics or later compilation phases.

This is adequate for a repository-local standard library, but it does not
solve the external-library requirement stated for Skald.

## Syntax and namespace construction

### Import forms

Niflheim implements all of these forms:

```nif
import a.b;
import a.b as b;
import a.b as x.y;
import a.b as .;

export import a.b;
export import a.b as b;
export import a.b as x.y;
export import a.b as .;
```

The imported module path and the path bound in the current module are distinct:

| Form | Local binding | Downstream surface |
|---|---|---|
| `import a.b;` | `a.b` | none |
| `import a.b as b;` | `b` | none |
| `import a.b as x.y;` | `x.y` | none |
| `import a.b as .;` | exported contents at the current root | none |
| `export import a.b;` | `a.b` | exported binding `a.b` |
| `export import a.b as b;` | `b` | exported binding `b` |
| `export import a.b as x.y;` | `x.y` | exported binding `x.y` |
| `export import a.b as .;` | exported contents at the current root | same flattened contents |

An alias renames the module binding, not any declaration and not its canonical
owner. After `import a.b as x;`, `x.Widget` still denotes `a.b::Widget`.

Imports may appear among other module items because parsing collects them
without an imports-first restriction. The resolver later treats them as one
module-wide set, so source order does not establish visibility.

### Export forms

`export` may prefix a class, interface, defined function, external-function
declaration, or import. A declaration without it remains absent from the
module's exported symbol table.

Niflheim therefore separates:

- locally declared symbols;
- directly exported symbols;
- locally bound imported modules;
- exported imported-module bindings; and
- symbols or module bindings flattened into a module root.

`SymbolInfo.owner_module_path` preserves the original definition through
re-export and flattening. This is an important invariant: a facade changes
visibility and spelling, not nominal type or callable identity.

### No selective symbol import

There is no `import a.b.Widget`, `from a.b import Widget`, or declaration-level
alias. The last path segment of an import is always a module file, not a symbol.
Callers either qualify through a module binding or rely on unqualified lookup.

### Unqualified lookup is broader than the binding syntax suggests

For functions, classes, and interfaces, unqualified lookup examines the
exported symbols of every directly imported module, independent of that
import's bind path. Therefore:

```nif
import left.math as left;
import right.math as right;
```

allows an unqualified exported name from either module when unique, and makes
the unqualified name ambiguous when both export it. The aliases control
qualified spelling but do not prevent namespace participation.

The policy is:

1. a matching local declaration wins;
2. otherwise, collect matching exported declarations from direct imports;
3. use the declaration when its canonical owner is unique;
4. report an ambiguity when multiple canonical owners match; and
5. report an unknown name when none match.

This is ergonomic in small modules, but it makes adding an export to a
dependency capable of breaking an existing consumer's unqualified lookup.
Explicit aliases do not insulate the consumer.

### Qualified lookup

Qualified lookup is import-rooted. A source cannot spell an absolute global
module name without first importing it.

The resolver uses the longest matching local bind-path prefix. Remaining
segments may traverse only exported imported-module bindings. The final
segment must be an exported declaration of the expected kind.

Plain dotted imports bind their full path:

```nif
import long.path.math;
long.path.math.add(1, 2);
```

They do not create an implicit `math` alias. Niflheim introduced explicit
aliases and later removed its earlier implicit-leaf behavior because same-leaf
module paths and canonical identity made that shortcut ambiguous. That
migration is documented in its proper-module-semantics plan and covered by
negative tests.

### Root flattening and collision rules

`as .` merges direct exported symbols and exported submodule bindings into the
current root. It enables standard-library facades such as `std.vec`, which
re-exports five implementation modules as one flat surface.

The resolver must consequently detect:

- two flattened symbols with the same leaf name;
- a flattened symbol colliding with a local declaration;
- a flattened symbol colliding with the root of a module binding;
- duplicate complete bind paths;
- different module bindings using the same complete path; and
- exported-flattened conflicts independently from local-only flattened
  conflicts.

Ordinary imports handle same-name exports lazily as an ambiguity at use.
Flattening handles them eagerly as a surface-construction error. This gives the
language two distinct conflict times depending on import style.

Multi-segment aliases add a second tree-shaped namespace that is independent
of physical module paths. Niflheim implements this correctly with longest
prefix matching, but the feature has little demonstrated use outside tests
compared with plain imports and one-segment aliases.

## Visibility

### Module visibility

Top-level declarations are private by default. Another module can reach one
only when:

- it imports a module surface that exports the declaration; or
- it imports a re-export chain whose visible endpoint includes the
  declaration.

Qualification never bypasses import or export checks. This prevents
"know-the-path" access to implementation details and makes the imported API
surface meaningful.

Re-exported declarations retain their defining owner. This prevents facade
modules from manufacturing new nominal classes or duplicate callable
identities.

### Class-member visibility

Niflheim separately supports `private` fields, methods, constructors, and
static fields. Its rule is class-private, not module-private:

- access is allowed only while checking a body declared in the exact owning
  class;
- subclasses cannot access inherited private members; and
- another class in the same module cannot access them.

Public class members do not require a second `export`; exporting the class
makes its non-private member surface usable.

This is a separate design dimension from module visibility. Skald can gain
meaningful codebase encapsulation with top-level module privacy before adding
member access modifiers.

### Source visibility versus native linkage

The `is_export` bit survives through semantic and backend IR. Exported
functions receive globally visible native labels, while private functions use
non-global canonical labels.

Whole-program compilation does not require this coupling. Source-public and
native-linker-public are different concepts, especially once packages,
dynamic libraries, foreign export, or separate compilation exist. Skald
should keep them distinct from the beginning.

## Graph loading and cycle policy

The resolver recursively loads imports from the entry module. A module cache
ensures each logical path is read and parsed once. A `visiting` set detects a
back edge and rejects the import cycle.

Only reachable modules enter `ProgramInfo`; unrelated files below the root are
ignored. A syntactically imported module is reachable even if none of its
exports are used.

Cycle rejection simplifies:

- recursive source loading;
- construction of re-exported surfaces;
- initialization ordering; and
- the absence of partially available module interfaces.

It is more restrictive than whole-program declaration resolution inherently
requires. With no top-level executable initialization and without recursive
re-exports, Skald could collect declarations for an import strongly connected
component before resolving bodies. If Skald rejects cycles initially, that
should be a deliberate language rule with focused tests, not an incidental DFS
limitation.

Niflheim contains cycle-detection code in graph loading and exported-surface
population, but its focused resolver suite currently has no direct import-cycle
test. This is a test gap, not evidence that cycles are accepted.

## Compiler architecture

### Resolver ownership

Niflheim's `resolve_program` currently owns:

1. entry/root path normalization;
2. source-file discovery and reading;
3. lexing and parsing every discovered file;
4. local symbol-table construction;
5. import graph loading and cycle detection;
6. imported and exported surface construction; and
7. an AST walk that validates some qualified visibility.

It raises `ResolveError` or lets lexer/parser exceptions stop compilation.
Diagnostics include source path, line, and column, but this is a fail-fast
exception path rather than a structured, multi-source diagnostic product.

This arrangement was expedient, but it blurs loader, frontend, and semantic
responsibilities. The resolver's AST visibility walk also does not replace
type checking; later code performs the authoritative kind-aware lookup.

Skald already has better boundaries to preserve:

- `SourceDatabase` can own many files and source-tagged spans;
- lexing and parsing return diagnostics rather than throwing source errors;
- diagnostics support primary and secondary labels across source IDs; and
- resolution is already the only phase that assigns semantic declaration
  identities.

A Skald module loader should discover and populate sources, but should not
absorb lexing, parsing, or semantic name resolution into filesystem code.

### Repeated lookup logic

Niflheim reconstructs related imported lookup in several layers:

- resolver visibility validation;
- type-check function/class/interface lookup;
- type canonicalization;
- semantic ID selection; and
- some symbol-index helpers.

All use the same program/module tables, but the loops and ambiguity behavior
are repeated. This creates drift risk when import semantics change. The
historical migration from implicit leaf aliases to strict qualification
illustrates that cost.

Skald should resolve every successful source name once to a typed identity.
Type checking and later lowering should consume that selection, not rediscover
an imported declaration from module tables and source strings.

### Whole-program semantic representation

Niflheim carries:

```text
ProgramInfo
  -> per-module type-check contexts
  -> SemanticProgram { entry_module, modules }
  -> LinkedSemanticProgram { ordered_modules, flattened classes/functions }
  -> BackendProgram
```

Canonical IDs contain their module path:

- `FunctionId(module_path, name)`;
- `ClassId(module_path, name)`;
- `InterfaceId(module_path, name)`;
- member IDs containing module path, owner type, and member name; and
- constructor IDs additionally containing an ordinal.

This allows identical leaf names in different modules and lets re-exported
symbols preserve nominal ownership. It also makes identities stable across
module traversal order.

The semantic linker sorts non-entry module paths lexicographically, appends the
entry module last, and concatenates declarations without deduplicating same
leaf names. Only the entry module's `main` is validated as the program
entrypoint. Other modules may define ordinary functions named `main`.

This is a major improvement over Niflheim's earlier behavior, which merged
cross-module declarations by leaf name and relied on bare native labels. Its
historical implementation plan shows that canonical identity had to be fixed
through the linker, function references, direct calls, metadata, tests, and
the ABI entry path. Skald should establish module ownership before any
cross-module consumer is implemented.

### Deterministic initialization

Niflheim has static class fields. Their explicit initializers run once before
user `main` in linked-module, class-declaration, and field-declaration order.
The linked module order is lexicographic for non-entry modules with the entry
last, not dependency-topological order. Current initializer restrictions
prevent inter-static references and arbitrary calls, so that order cannot
express a dependency.

Skald has no static state today. Module loading should therefore avoid
prematurely freezing an initialization order. If module or static
initialization is later introduced, its dependency, cycle, failure, and
destruction rules need their own language design.

## Code generation and linkage

### Canonical internal symbols

Niflheim derives internal function, class, interface, method, constructor, and
static symbols from canonical semantic IDs. Calls and function values use the
same symbol table. A distinct ABI-visible `main` wrapper or alias selects the
entry module's canonical function.

The separation of semantic entry identity from the host entry symbol is
directly applicable to Skald. Skald already emits its source `main` as an
internal dense-ID symbol and a small public process wrapper, so its backend is
well-positioned for multiple modules.

### Mangling collision found by this audit

Niflheim's `_mangle_fragment` replaces `.`, `:`, `[`, and `]` with `_`.
That transformation is not injective. For example:

```text
FunctionId(("a", "b"), "f")  -> __nif_fn_a_b__f
FunctionId(("a_b",), "f")    -> __nif_fn_a_b__f
```

Both logical module paths are legal. The backend symbol registry detects the
duplicate and fails rather than silently mislinking, but a valid source program
can still be rejected at codegen. The inspected symbol tests do not cover this
module-separator collision.

Skald should not encode canonical identity with lossy character replacement.
Whole-program dense IDs already give it collision-proof private symbols.
Any future stable or externally visible mangling should use an injective
length-prefixed or properly escaped encoding with direct collision tests.

### External functions

An external function still has a module-qualified semantic `FunctionId`, but
its direct native call uses the bare declared foreign name. The historical
module work deliberately stopped merging an external declaration in one
module with a body in another.

Two modules declaring the same foreign symbol therefore have distinct
semantic functions but request the same backend symbol. The backend's general
symbol registry treats different owners of one symbol as a conflict. There is
no focused policy or test for compatible repeated external declarations.

Skald's living module document already identifies external-declaration
coalescing as open. Whole-program module design should settle it explicitly.
A useful initial rule would allow identical foreign symbol/signature
assertions to share one link target while diagnosing incompatible assertions;
it should not merge a Skald definition with an external declaration merely
because their source leaf names match.

## Standard-library evidence

Niflheim's standard library proves several practical properties:

- ordinary private-by-default modules can expose stable public surfaces;
- standard modules can depend on other standard modules without compiler
  built-ins;
- consumers can use the same imports for source-defined classes, interfaces,
  and functions;
- an explicit alias is useful for frequently qualified function modules such
  as `std.math`; and
- root flattening can build a facade, as `std.vec` does over several generated
  vector implementation modules.

It also shows that re-export and flattening are not prerequisites for most
library code. In the audited repository, the notable production use of
`export import ... as .` is the `std.vec` facade. Most standard, sample, and
project imports are plain imports; explicit one-segment aliases are used where
qualified spelling would otherwise be long or collide with ordinary names.
Multi-segment aliases are primarily exercised by tests.

Skald can support a future standard library without implementing the whole
facade surface in its first module slice.

## Strengths worth preserving

### Import-rooted access

An import is required even for qualified access. This makes dependencies
explicit and prevents absolute global lookup from bypassing API boundaries.

### Private by default

A new top-level declaration does not silently become part of a module's
consumer-visible API. This is a sound default for both application structure
and future libraries.

### Canonical owner through re-export

Visibility does not change identity. This is essential for nominal types,
method dispatch, metadata, diagnostics, and code generation.

### Same leaf names across modules

Module identity participates in canonical identity, so modules are real
namespaces rather than load-order filters over one global namespace.

### Entry module is explicit

The input file chooses the entry module. Only its `main` has entrypoint
semantics; all other functions named `main` are ordinary functions.

### Closed-world compilation

The reachable graph is known before semantic lowering and code generation.
This supports Skald's initial whole-program constraint and enables global
verification and optimization without defining object-file interfaces.

## Costs and limitations

### Physical path, logical identity, and package identity are conflated

This is the largest limitation for Skald's external-library goal. A future
package abstraction would have to reinterpret identities currently baked into
types, symbols, diagnostics, and metadata.

### Alias imports do not isolate unqualified names

Every direct import participates in unqualified lookup. Dependency API growth
can therefore create new ambiguities even when the consumer consistently uses
aliases.

### Re-export and flattening are a large semantic surface

They require recursive surface construction, conflict policy, longest-prefix
module traversal, canonical-owner preservation, and tests across parsing,
resolution, type checking, lowering, and codegen. Their value is real but
concentrated in facade modules.

### Cycle rejection is coupled to the loader

The language cannot distinguish harmless declaration cycles from executable
initialization cycles because the loader rejects all cycles before semantic
analysis.

### Resolution is fail-fast and phase-mixed

One missing module or namespace conflict prevents collecting independent
errors from other loaded sources. Filesystem failures, syntax failures, graph
errors, and semantic visibility failures do not share one structured
diagnostic model.

### Export has two meanings

It controls source visibility and native symbol export. Those responsibilities
will diverge under separate compilation, foreign exports, or dynamic
libraries.

### Deterministic order is not dependency order

Lexicographic module ordering is deterministic, but it is not a semantic
initialization dependency order. Niflheim avoids observable problems through
current static-initializer restrictions.

### External declaration repetition is unresolved

Canonical semantic ownership and raw foreign symbols pull in different
directions when several modules describe the same external function.

### No dependency provenance

Diagnostics can name a module path and source path, but cannot say which
package, version, dependency alias, or source root supplied it.

## Implications for Skald's current compiler

### Existing assets

Skald already has several prerequisites that Niflheim had to retrofit:

- `SourceDatabase` supports multiple source files and source-tagged spans;
- structured diagnostics can label declarations in different files;
- resolution assigns dense `FunctionId`, `ClassId`, `InterfaceId`, and member
  identities;
- resolved, HIR, and MIR programs are already whole-program flat tables;
- later phases use IDs rather than source names;
- internal backend symbols are derived from dense IDs and are collision-proof
  within one program; and
- the ABI `main` wrapper is separate from the internal entry function.

The module design should extend these properties instead of replacing them
with string-qualified IDs.

### Required changes

The current pipeline assumes one source-shaped AST and one top-level symbol
map. A module system requires at least:

- a loader-owned graph of logical module identities and source IDs;
- one parsed compilation unit per module;
- module ownership on declarations and source name uses;
- module-scoped top-level symbol tables and public surfaces;
- an import-aware resolver that assigns existing dense declaration IDs across
  the whole graph;
- an entry-module identity distinct from `entry_function`;
- deterministic cross-module declaration-ID allocation and dump order;
- multi-source driver I/O and diagnostics;
- cross-module golden fixtures and public API tests; and
- documented source-root, standard-library, and dependency behavior.

HIR and MIR do not need to perform name lookup. They may need `ModuleId` or
owner metadata for dumps, diagnostics, future visibility verification, and
stable public names, but their executable references can continue to use the
existing dense IDs.

### Avoid an unnecessary semantic linker

Because Skald already constructs one flat resolved program, it does not need
to copy Niflheim's sequence of per-module semantic programs followed by a
separate concatenating linker. Resolution can allocate global dense IDs in a
deterministic module order, retain per-module ranges or ownership tables, and
produce one whole-program resolved graph directly.

This keeps one authority for declaration identity and avoids a later phase
that must rediscover duplicates or rewrite IDs.

## Recommended first-version Skald boundary

The following is a design recommendation, not a frozen contract.

### Include

- One `.ska` file per module.
- Module identity supplied by the loader rather than a source declaration.
- One selected entry module and its reachable imports.
- Whole-program lexing, parsing, resolution, type checking, MIR lowering,
  verification, and code generation.
- Module imports with full qualification.
- An optional explicit one-segment module alias for ergonomic qualification.
- Private-by-default top-level functions, classes, interfaces, and external
  declarations.
- One explicit public marker on top-level declarations.
- Same leaf declaration names in different modules.
- Import-rooted qualified lookup.
- The existing `fn main() -> i64` rule applied only to the entry module.
- Canonical package/module ownership metadata plus compilation-wide dense
  declaration identities.
- Explicit project, standard-library, and dependency source provenance in the
  loader model, even if the first CLI exposes only a subset.
- Deterministic diagnostic, dump, and emission order.

### Defer

- Re-exported imports.
- `as .` or wildcard/root flattening.
- Multi-segment import aliases.
- Unqualified imported-symbol fallback.
- Selective symbol imports and symbol aliases.
- Source-declared module names.
- Directory modules or implicit index files.
- Top-level variables and module initialization.
- Separate compilation, object/module interface files, and binary libraries.
- A package manager, registry, or version solver.
- Native export of source-public Skald declarations.
- Member-level visibility changes unless separately designed.

This boundary deliberately makes every cross-module dependency visually
explicit:

```ska
import std.io as io;
import app.model;

fn main() -> i64 {
    io.println_i64(app.model.answer());
    return 0;
}
```

Exact tokens and whether plain imports bind a full path or a leaf remain open.
If a short binding is desired, requiring an explicit alias avoids Niflheim's
implicit-leaf migration problem.

## Recommended internal model

### Separate three identities

Skald should model these as distinct facts:

```text
PackageId
  identifies the workspace package, standard library, or one dependency

ModuleId
  identifies one logical module within a package

SourceId
  identifies the concrete source text used for this compilation
```

A loader table maps `(PackageId, logical module path)` to exactly one
`SourceId` and physical path. Imports should resolve through an explicit
package/root configuration, not an ordered first-match filesystem search.
Duplicate candidates should be diagnosed rather than silently shadowed.

The first CLI does not need a package manager to establish this boundary. It
can construct a compilation request from:

- the entry source and project root;
- a compiler-configured standard-library root; and
- zero or more explicitly named dependency roots.

The representation leaves room for a later manifest without changing nominal
type identity.

### Preserve dense semantic IDs

Keep `FunctionId`, `ClassId`, `InterfaceId`, and member IDs dense and
compilation-local. Add owning `ModuleId` to declaration records or a canonical
ownership table. Allocate IDs after graph discovery in a documented stable
order, for example canonical package/module order followed by source
declaration order.

Benefits:

- no lossy native mangling;
- compact tables and fast later-phase lookup;
- minimal HIR, MIR, verifier, and backend churn;
- deterministic dumps; and
- no dependency of semantic equality on display strings.

### Add a loader phase without collapsing frontend phases

A useful orchestration is:

```text
compilation request
  -> discover module graph and populate SourceDatabase
  -> lex each source
  -> parse each source into a module AST
  -> collect all module declarations and public surfaces
  -> resolve imports, types, members, and bodies to dense IDs
  -> type check one whole resolved program
  -> existing HIR, MIR, passes, and backend
```

Discovery must parse imports to find the graph. This does not require the
filesystem loader to own general parsing: orchestration can lex/parse each
newly discovered source, retain its products and diagnostics, extract imports,
and ask the loader only to map logical imports to sources.

Malformed modules should not enter semantic phases, but independent discovered
sources can still contribute diagnostics before the driver stops.

### Centralize imported lookup

Resolution should expose one import-aware lookup service per module that:

- distinguishes module bindings from declarations;
- enforces public visibility;
- produces typed declaration IDs;
- reports ambiguity or wrong-kind errors with cross-file labels; and
- records the chosen canonical owner.

Body resolution, type syntax, inheritance, interface claims, and construction
should use that service. Type checking and lowering should receive resolved
IDs and never repeat imported string lookup.

## Design decisions to settle before an implementation roadmap

| Priority | Decision | Recommendation from the audit |
|---|---|---|
| 1 | What is canonical nominal identity? | `(PackageId, ModuleId, declaration)` conceptually; dense compilation-local IDs operationally. |
| 1 | How are standard and external libraries located? | Explicit package/root map with provenance and duplicate detection, not one undifferentiated root. |
| 1 | What does an import bind? | A module only; full path by default and optional explicit one-segment alias. |
| 1 | Can imports create unqualified declarations? | No in the first version. Qualification keeps lookup stable and the implementation small. |
| 1 | What is public? | Top-level declarations are private by default; one explicit public marker exposes them to importers. |
| 1 | Are source-public symbols native-public? | No. Native visibility is a separate linkage decision. |
| 1 | Which module owns entry semantics? | The explicitly selected entry module only. |
| 2 | Are import cycles legal? | Prefer allowing declaration-only cycles if re-exports and initialization are deferred; otherwise freeze and test deliberate rejection. |
| 2 | How are IDs and dumps ordered? | Canonical package/module order, then source declaration order, independent of import traversal order. |
| 2 | May several modules declare one foreign symbol? | Coalesce identical symbol/signature assertions; diagnose incompatible ones; never merge with a Skald body by leaf name. |
| 2 | What path spelling appears in diagnostics? | Stable logical package/module name plus concrete source path where useful. |
| 2 | Can a package shadow `std`? | No accidental first-match shadowing; the standard package identity should be explicit. |
| 3 | Are re-exports needed initially? | Defer until a concrete facade need outweighs the recursive surface complexity. |
| 3 | Is root flattening needed? | Defer; it is the highest-conflict part of Niflheim's surface. |
| 3 | Is member visibility part of the module slice? | Keep separate unless the language design deliberately adds class-private access at the same time. |
| 3 | What remains in later IRs? | Dense selected IDs plus only the module metadata needed for diagnostics, dumps, and future ABI work. |

## Proposed validation themes

A later design and roadmap should require coverage for:

- entry modules at the root and in nested module paths;
- project, standard-library, and dependency imports;
- missing modules and duplicate candidates across roots;
- entry files outside the selected project root;
- same leaf declarations and same leaf modules in different paths/packages;
- full-path and explicit-alias qualification;
- private declaration access through both short and fully qualified spellings;
- import-required behavior for known absolute paths;
- local and module-binding name collisions;
- import cycles according to the chosen policy;
- cross-module forward types, inheritance, interfaces, calls, ownership, and
  lifecycle behavior;
- repeated compatible and incompatible external declarations;
- entry-module `main` versus non-entry functions named `main`;
- deterministic IDs, dumps, diagnostics, assembly, and native behavior under
  import-order changes;
- mangling collisions such as `a.b` versus `a_b`;
- multi-file diagnostic rendering with labels in both importer and exporter;
- malformed or unreadable imported sources;
- reachable-only compilation and unused imported modules;
- source-root and symlink containment policy; and
- end-to-end standard-library access without compiler-recognized declaration
  names.

## Final assessment

Niflheim's module system is semantically credible and proven by substantial
real code. Its best contribution to Skald is not its full syntax; it is the
lesson that module ownership must become canonical before lookup, linking,
metadata, entrypoint selection, and symbols diverge.

Skald can reach that invariant with less machinery because it already has
multi-source-capable spans, structured diagnostics, dense semantic IDs, flat
whole-program IR, and a separate ABI entry wrapper. The design should exploit
those assets, introduce package-aware source provenance early, and postpone
namespace-merging conveniences until ordinary qualified modules and a real
Skald standard library demonstrate the need.
