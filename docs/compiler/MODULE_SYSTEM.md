# Module-System Compiler Contract

Status: authoritative implemented compiler contract for multiple-file
whole-program compilation, module providers, filesystem resolution, entry
selection, identities, loading, diagnostics, determinism, and linkage. The
module layer selects logical or positional entries, constructs outside-root
singletons, loads and parses only the reachable closure, allocates canonical
graph and semantic identities, accepts cyclic multi-module dependencies,
rejects direct self-imports, enforces imports and visibility, and coalesces
compatible cross-module external declarations. The request pipeline and CLI
compile either entry form through this graph. The
in-memory source-text convenience API remains available without filesystem
discovery; module-bearing sources require a `CompilationRequest`. Driver and
artifact behavior remains authoritative in
[Driver and Artifacts](DRIVER_AND_ARTIFACTS.md), while
[Modules and Foreign Interoperation](../language/MODULES_AND_INTEROP.md)
owns source-visible module semantics.

## Scope

The initial module system compiles one complete reachable program. It has no
separate library compilation, cached or serialized public interface, module
object, manifest, registry, version solver, or link-time Skald module
discovery.

A compilation request contains:

```text
CompilationRequest
  entry: FilePath | ModulePath
  module roots: zero or more anonymous filesystem roots
  standard library: default root | replacement root | disabled
  target and artifact options
```

The entry module and every module reached recursively through imports form the
program. Unrelated files below configured roots are ignored. Importing a module
makes it reachable even if none of its declarations is used.

## Providers and logical module resolution

A provider maps logical module paths to source files. The initial provider
kinds are:

- an anonymous filesystem root;
- the active standard-library root; and
- a singleton positional-file entry when that file is outside every root.

A filesystem root maps `a::b` to `<root>/a/b.ska`. Roots are lookup locations,
not source-visible prefixes, and command-line options never bind or rename
them. All providers form an unordered union. Root order has no precedence.

Discovery probes the exact path requested by an import or logical entry. It
does not scan or parse every source below a root. Exact-case verification may
enumerate the requested path's individual directory components, but complete
root contents are not semantic input.

Partial overlap is valid:

```text
/project/modules/math/trigonometry.ska -> math::trigonometry
/deps/modules/math/geometry.ska        -> math::geometry
```

The shared `math` prefix has no owner and conveys no package relationship.
For one exact logical path, resolution has only these outcomes:

| Candidates after root normalization | Result |
|---:|---|
| Zero | Missing module. |
| One | Load that module. |
| More than one from distinct providers | Ambiguous module; report every candidate. |

Content equality, hard links, and common symlink targets never resolve an
ambiguity. Root order never selects a winner.

## Root and filesystem normalization

Each configured root is normalized before provider construction:

1. Resolve a relative root against the compiler process's current working
   directory.
2. Canonicalize the root itself.
3. Require the result to exist and be a directory.
4. Coalesce all options naming that canonical directory into one provider,
   while retaining their configuration provenance.

This coalesces spellings such as `modules`, `./modules`, and a symlink to the
same root. It also coalesces an ordinary root and the standard-library root
when they name the same directory. Diagnostics select a deterministic display
spelling independent of option order.

File and directory symlinks below a root are followed, including targets
outside the root. A root is not a filesystem sandbox. The logical module path
comes from the lexical path visible below the root:

```text
/modules/math/geometry.ska -> /shared/geometry.ska
```

still provides `math::geometry`. Broken or cyclic symlinks, unreadable
candidates, and candidates that do not resolve to regular files are diagnosed
when reached.

Module components use exact case on every host. `std::Str` does not resolve
`std/str.ska`. A case-insensitive host must verify actual component spellings
and diagnose collisions it cannot distinguish deterministically.

Canonical and display paths have deliberately different roles:

- each root records a canonical directory for equivalence plus deterministic
  user-facing spelling information;
- each module records its logical path, provider, lexical root-relative path,
  display source path, and `SourceId`; and
- a module candidate's canonical target may be retained for I/O, caching, or
  diagnostics, but never as semantic identity or an ambiguity key.

Different logical paths always create different module instances, even if
they reach the same physical source:

```text
/root-a/math/geometry.ska -> /shared/geometry.ska -> math::geometry
/root-b/geometry.ska      -> /shared/geometry.ska -> geometry
```

These mappings receive distinct `SourceId`, `ModuleId`, AST, declaration, and
nominal type identities. Shared byte caching is permitted; semantic products
must remain separate.

## Standard-library provider

The compiler installation supplies a default standard-library root.

- `--stdlib-root <directory>` replaces that default.
- `--no-stdlib` disables it.
- The options are mutually exclusive.
- `--stdlib-root` may occur only once.

The standard library has no lookup precedence and does not rename `std`.
Standard modules are loaded only when reachable and use ordinary import,
visibility, semantic, and code-generation rules. An ordinary root may provide
other modules below `std`; an exact collision with the standard-library
provider is ambiguous.

Future language items may designate a logical path such as `std::Str`. The
owning feature records whether that path denotes a module or declaration and
resolves it once to the corresponding semantic identity. Lowering and backends
must not repeatedly compare source path strings.

## Entry selection and command line

The CLI requires exactly one entry selector:

```text
skac path/to/main.ska [module options]
skac --entry app::main [module options]
```

The positional form and `--entry` are mutually exclusive. More than one
positional file, repeated `--entry`, both forms, or neither form is a usage
error. `--entry` accepts one canonical `::`-separated module path and resolves
it through the provider union.

`--module-root <directory>` is repeatable. Zero explicit roots is valid.
Root normalization and ambiguity rules are independent of option order.

### Positional file identity

A positional `.ska` file receives its identity as follows:

| Location | Module identity |
|---|---|
| Lexically below exactly one normalized provider | Its ordinary root-relative module path. |
| Lexically below multiple distinct providers | Ambiguous entry identity, regardless of logical path or physical target. |
| Outside every root | A singleton top-level module named by the file stem. |

Containment is lexical after making the entry spelling absolute and removing
ordinary `.` and `..` components. It is checked against the canonical root and
the normalized configured root spellings retained for that provider. This
makes `/alias/a.ska` and `/real/a.ska` equivalent when `/alias` is the
configured symlink spelling of canonical root `/real`. The compiler does not
canonicalize descendant file targets to infer containment: if an in-root
symlink reaches an otherwise outside file, selecting the outside spelling
does not give it the symlink's rooted identity.

A singleton provider exposes exactly the selected file:

```text
skac /tmp/my_main.ska

my_main -> /tmp/my_main.ska
```

It does not expose `/tmp/keso.ska`. The stem must be a valid module component.
The singleton participates in ordinary resolution and may be named by an
import. A distinct provider of `my_main` is ambiguous. The singleton cannot
import itself directly, but another reachable module may import it as part of
an ordinary multi-module cycle.

Selecting a rooted file and selecting its logical path intern the same
`ModuleId`. Importing the selected module later does not load it twice.

### Output defaults

`-o` and `--output` continue to override output selection. A positional entry
retains the current input-stem location:

| Invocation | Executable | `--emit asm` |
|---|---|---|
| `skac app/main.ska` | `app/main` | `app/main.s` |

A logical entry uses its final module component in the current directory:

| Invocation | Executable | `--emit asm` |
|---|---|---|
| `skac --entry app::main` | `main` | `main.s` |
| `skac --entry tools::formatter` | `formatter` | `formatter.s` |

Existing artifact publication and input/output-alias protections apply
unchanged.

## Identities and phase ownership

The compiler keeps these concepts separate:

| Identity | Responsibility |
|---|---|
| `PackageId` | Source ownership/provenance for an application or workspace, standard library, dependency, or singleton entry; initially request-local and anonymous where no stable identity exists. |
| `ModulePath` | Canonical source-visible logical address. |
| `ModuleId` | Dense request-local identity of one loaded module. |
| `SourceId` | Source instance owning source spans. |
| `ProviderId` | Provider supplying the module mapping. |
| Declaration IDs | Existing dense request-local function, class, interface, member, and other semantic identities. |

Every declaration retains its owning `ModuleId`. Package identity is not
inferred from a module path prefix. A later manifest may assign stable package
identities or group roots without changing source imports. Any future
package-private visibility compares `PackageId`, never the first component of
`ModulePath`.

Resolved IR, checked HIR, and MIR each carry one `ProgramModuleTable`. The
table stores dense `ModuleProvenance` entries in `ModuleId` order and the
selected entry `ModuleId`. Its constructor rejects non-dense identities,
duplicate logical paths, and an unknown selected entry. Top-level function,
class, and interface declarations store their owning `ModuleId`; class and
interface members derive module ownership through that enclosing declaration.
Existing declaration and definition tables remain flat and keep their typed
semantic identities.

The one-AST `resolve::resolve` API is a compatibility adapter. It synthesizes a
request-local `main` module owned by `m0`, provider/package zero, and the AST's
existing `SourceId`, then uses the same program-resolution stage as
`resolve::resolve_module_graph`. The graph entry collects every reachable
module in canonical logical-path order and allocates top-level and member
identities in source order within each module. It records one private-by-
default declaration index and direct public surface per module, selects
`main` only from the graph's selected entry module, and produces one flat
resolved program for the existing type-check, HIR, MIR, verification, and
backend phases.

At the current implementation boundary, each direct module import creates one
exact qualified binding: the complete canonical module path by default, or one
identifier when aliased. Multiple distinct bindings may select the same loaded
`ModuleId`; repeated or conflicting local bindings are rejected. Qualified
lookup resolves the binding and one public declaration leaf centrally across
types, signatures, hierarchy, interface claims, calls, construction,
allocation, casts, and type tests. Absolute paths, ancestors, descendants, and
transitive imports grant no access without an exact direct binding. The
resolved program records local bindings and canonical targets, while HIR and
lower phases retain only selected declaration identities.

Each selective item is resolved directly against the canonical import-source
module's owned declaration index. A valid item adds its source name or explicit
alias to a separate ordinary-binding table and retains the original dense
declaration identity and canonical owner. Collection rejects missing and
private targets, names not owned directly by the target, repeated local
ordinary names, and collisions with declarations owned by the importer.
Selective bindings neither create a module binding nor mutate a target public
surface, so they cannot re-export declarations. Parameters and lexical locals
continue to shadow imported ordinary names under the existing lexical rules.
Request compilation feeds this loaded graph into semantic compilation. The
in-memory source-text adapter continues to synthesize one singleton semantic
module without filesystem lookup.

One canonical `ModulePath` resolves to at most one loaded `ModuleId`. Multiple
module aliases and selective imports of that module reuse its `ModuleId`,
`SourceId`, parsed product, declaration tables, and graph node. They do not
repeat source I/O, lexing, parsing, or semantic collection.

The compilation proceeds as follows:

```text
normalize providers and select the entry
  -> lex and parse the entry
  -> resolve imports and parse newly reached modules to graph closure
  -> reject direct self-imports
  -> collect per-module declarations and public surfaces
  -> resolve bindings and uses to dense identities
  -> type check one whole program
  -> existing whole-program HIR, MIR, verification, backend, and emission
```

Loading owns provider normalization, path mapping, I/O, source-instance
creation, provenance, and graph construction. It does not select declarations.
Resolution is the only import-aware declaration-selection phase. Type checking
and lower phases consume selected identities rather than repeat path or name
lookup.

`SourceDatabase` remains request-local and stores every loaded source. The
module graph records the selected entry, canonical path and provenance of each
module, its possibly cyclic direct imports and local bindings, and
deterministic graph order. Graph shape does not affect dense identity
allocation: modules remain ordered by canonical logical path.

Lexing and parsing produce one retained phase product per module instance. An
unreadable or malformed imported module is an ordinary source failure with
structured diagnostics, never an internal compiler error. No erroneous source
product advances into later semantic phases.

Resolution exposes one centralized import-aware lookup service per current
module. It:

- handles declarations owned by the current module;
- recognizes only directly imported module or selective bindings;
- enforces public visibility and the declaration kind required by the use;
- returns the existing dense declaration identity; and
- produces cross-file labels for private, ambiguous, missing, and wrong-kind
  uses.

Resolution may produce flat whole-program declaration tables with module
ownership. There is no separate semantic linker. HIR, MIR, verification, and
backends never repeat import or declaration lookup from source strings.

## Determinism and linkage

Discovery order, import order, and provider-option order must not affect
identities, diagnostics, dumps, generated symbols, or emitted code. Dense
identities are allocated in canonical module-path order, then source
declaration order, then existing member order.

The selected entry is explicit request and graph metadata, not a special
module kind or allocation position. Selecting a different entry without
changing the reachable module set does not reorder module or declaration
identities.

`ProgramModuleTable` construction preserves its supplied canonical entry
order independently of the selected entry. All semantic phases clone that
table and declaration owners without reallocation. Resolved, HIR, and MIR
dumps expose the selected module, dense module provenance, and top-level
owners. MIR verification rejects malformed table metadata, unknown
declaration owners, and an entry function whose owner differs from the
selected module.

Source-public declarations remain private compiler-generated native symbols.
Only the selected entry function feeds the host-visible `main` wrapper.
Internal symbols use collision-free semantic identities rather than lossy
module-path encodings.

Valid external declarations in different modules retain separate source and
function identities. Declarations of the same exact foreign symbol and ABI
signature coalesce to one compilation-wide external-link identity. An
incompatible signature is rejected before backend emission with every
conflicting declaration labeled and the signature difference described.
Duplicate external declarations within one module remain ordinary duplicate
top-level errors, and a Skald function definition never coalesces with an
external declaration.

The implemented resolver allocates dense external-link identities in exact
foreign-symbol order after resolving all source signatures. One immutable
table owns each symbol and its ordered source `FunctionId` declarations.
Resolved, HIR, and MIR declarations carry only the corresponding
`ExternalLinkId`; the table crosses those phases unchanged. MIR verification
checks table density, symbol uniqueness, complete bidirectional membership,
signature agreement, and internal/external separation before the backend
selects the native symbol from the table.

## Required diagnostic coverage

Implementation must cover at least:

- missing, repeated, or conflicting entry selectors;
- invalid logical-entry spelling;
- repeated `--stdlib-root`, or conflicting `--stdlib-root` and
  `--no-stdlib`;
- invalid module roots, standard-library roots, entry paths, `.ska` suffixes,
  file stems, or module components;
- a positional entry with multiple possible logical identities;
- a missing imported module or logical entry module;
- one exact logical module supplied by multiple distinct normalized providers,
  regardless of candidate contents or physical identity;
- module-path case mismatches and host case collisions;
- broken or cyclic symlinks, unreadable candidates, and candidates that do not
  resolve to regular files;
- repeated or conflicting module bindings and selective-import bindings;
- alias-based import-source mistakes, multi-segment module aliases, trailing
  selective-import commas, and wildcard imports;
- private cross-module access and qualified use without a direct import;
- missing or wrong-kind qualified declarations;
- unreadable or malformed imported sources as source diagnostics;
- direct self-imports;
- incompatible cross-module external ABI declarations, with every declaration
  labeled and the signature difference described; and
- an invalid or missing selected entry function.

Diagnostics and identity allocation are deterministic. Ambiguity and
visibility diagnostics label the importing use and every relevant provider
candidate or private declaration across source files. Self-import diagnostics
label the redundant direct import. ABI diagnostics label every conflicting
declaration.

## Complete example

Given:

```text
/project/modules/app/main.ska
/project/modules/app/model.ska
/deps/modules/math/geometry.ska
/sdk/modules/std/numeric.ska
```

the providers compose this logical tree:

```text
app::main
app::model
math::geometry
std::numeric
```

The supporting modules may declare:

```ska
// app/model.ska
public fn origin() -> i64 {
    return 0;
}

// math/geometry.ska
public fn measure(value: i64) -> i64 {
    return value;
}

// std/numeric.ska
public fn identity(value: i64) -> i64 {
    return value;
}
```

The entry module can mix selective and qualified imports:

```ska
// app/main.ska
from app::model import origin;
import math::geometry as geometry;
from std::numeric import identity;

fn main() -> i64 {
    return identity(geometry::measure(origin()));
}
```

and build with:

```text
skac --entry app::main \
    --module-root /project/modules \
    --module-root /deps/modules \
    --stdlib-root /sdk/modules
```

Moving a provider directory changes only build configuration. Adding another
provider for `math::geometry` makes the build ambiguous; it never shadows the
existing candidate.

## Deferred work

The frozen initial contract excludes:

- source-declared module or package names;
- command-line aliases that rename module trees;
- package-private visibility and package distribution;
- manifests, registries, versions, and lockfiles;
- separate compilation, serialized interfaces, and binary Skald libraries;
- directory modules and implicit index files;
- eager compilation of all files below a root;
- native export of source-public declarations; and
- module initialization or initialization order.

Source-level exclusions such as relative imports, wildcards, re-exports, and
multi-segment aliases are owned by the
[language contract](../language/MODULES_AND_INTEROP.md#initial-module-system).

## Implementation status

The initial module system is implemented across the CLI, driver request,
provider normalization, reachable graph loader, frontend, resolver, semantic
IRs, verifier, backend, diagnostics, and end-to-end fixtures. This contract
has no remaining source, provider, filesystem, entry, graph, identity,
visibility, linkage, or diagnostic design decisions. Future work that changes
these rules requires an explicit language or compiler contract revision.
