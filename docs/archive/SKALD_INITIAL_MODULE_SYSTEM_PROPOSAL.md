# Initial Skald Module-System Design Record

Status: historical design record. Its decisions were promoted into the frozen
[language](../language/MODULES_AND_INTEROP.md#initial-module-system)
and [compiler](../compiler/MODULE_SYSTEM.md) contracts. Those living documents
are authoritative.

This proposal turns the
[Niflheim module-system audit](MODULE_SYSTEM_NIFLHEIM_AUDIT.md) into a small
initial module system for Skald. It incorporates the subsequent decisions
about multiple anonymous module roots, split logical namespaces, file and
logical entry selection, and singleton entry modules.

The current compiler still accepts one source file. The
[status matrix](../language/STATUS.md) distinguishes that implementation
boundary from the frozen multiple-file design.

## Intended outcome

The first module system should provide:

- whole-program compilation from one explicitly selected entry module;
- one source file per logical module;
- access to application, standard-library, and external-library modules;
- private-by-default top-level declarations with explicit public visibility;
- stable imports that do not depend on command-line names for source roots;
- deterministic resolution across multiple source locations;
- explicit qualified or selective cross-module use without wildcard,
  re-export, or namespace-flattening machinery; and
- an identity model that can later support packages and separate compilation
  without replacing compilation-local dense declaration IDs.

## Core model

| Term | Meaning |
|---|---|
| **module path** | A non-empty `::`-separated sequence of identifiers, such as `app::model` or `std::io`. It is the stable source-visible address of one module in a compilation request. |
| **module** | One logical compilation instance of a `.ska` source mapping, loaded under exactly one module path. The same underlying file may back another, distinct module path. |
| **module root** | A configured filesystem directory mapping relative `.ska` paths to logical module paths. It is a lookup location, not a source-visible namespace binding. |
| **module provider** | A compiler input contributing module-path-to-source mappings. Filesystem roots and singleton file entries are provider kinds. |
| **package identity** | Source provenance and ownership, distinct from module paths and not inferred from their first segment. |
| **entry module** | The ordinary module selected by the build as the source of the program entry function. |
| **reachable graph** | The entry module and every module reached recursively through imports. This is the complete first-version program. |

The central separation is:

```text
source code names logical modules
build configuration locates those modules
compiler metadata records who supplied them
```

A command-line path may change without changing source imports. A shared
logical prefix does not imply common package ownership.

## Source files and module paths

There is no source `module` declaration initially. A filesystem root derives
module paths from relative file paths:

```text
<root>/app/main.ska          -> app::main
<root>/math/geometry.ska     -> math::geometry
<root>/std/collections.ska   -> std::collections
```

Every path component and the file stem must be a valid Skald identifier. The
`.ska` suffix is not part of the logical path. Logical paths are
case-sensitive; the loader must diagnose host-path collisions rather than
make source meaning depend on host filesystem behavior.

Directories introduce path prefixes, not declaration-bearing modules. A
module and descendants of the same path may therefore coexist:

```text
<root>/math.ska              -> math
<root>/math/geometry.ska     -> math::geometry
```

There are no directory modules or implicit index files initially.

## Module roots and logical-tree composition

### Anonymous lookup locations

Roots do not receive arbitrary source-visible names:

```text
skac --entry app::main \
    --module-root /project/modules \
    --module-root /deps/modules \
    --stdlib-root /sdk/modules
```

may locate:

```text
/project/modules/app/main.ska       -> app::main
/deps/modules/math/geometry.ska     -> math::geometry
/sdk/modules/std/io.ska             -> std::io
```

Source imports `app::main`, `math::geometry`, or `std::io`. It never imports a
command-line alias for one of those directories. If a later manifest declares
a stable package name, a `name=path` CLI form may assert that identity; it
must not arbitrarily rename the library for each build.

### Unordered union and split namespaces

All providers compose into one logical tree. Partial prefix overlap is valid:

```text
/project/modules/math/trigonometry.ska -> math::trigonometry
/deps/modules/math/geometry.ska        -> math::geometry
```

The `math` prefix has no single owner. These modules may have different
package identities, and future package-private visibility must compare those
identities rather than the text `math`.

Resolving one exact module path has three outcomes:

| Candidates across all providers | Result |
|---:|---|
| Zero | Missing-module diagnostic. |
| One | That candidate defines the module. |
| More than one candidate from distinct normalized providers | Ambiguous-module diagnostic naming every provider and candidate. |

Root order never selects a winner. Reordering roots must not change a
successful program, declaration IDs, diagnostics, dumps, or emitted code.

Canonical deduplication applies to equivalent root configurations, not to
individual module candidates. After roots have been normalized and equivalent
roots coalesced, two distinct providers supplying the same exact logical path
are always an ambiguity. The result does not depend on whether their
candidates have equal contents, share a hard link, or resolve through symlinks
to the same target.

Conversely, different logical paths always denote different module instances,
even when they use the same underlying source:

```text
/root-a/math/geometry.ska -> /shared/geometry.ska -> math::geometry
/root-b/geometry.ska      -> /shared/geometry.ska -> geometry
```

Both mappings are valid. They receive distinct module, source, declaration,
and nominal type identities. An implementation may cache shared source bytes,
but it must not merge their parsed or semantic products.

### Filesystem normalization and display paths

A configured filesystem root is normalized before provider construction:

1. Resolve a relative spelling against the compiler process's current working
   directory.
2. Canonicalize the root directory itself, require it to exist, and require it
   to be a directory.
3. Coalesce root options whose canonical directory is identical, including an
   ordinary and standard-library root that name the same directory. Preserve
   their configuration provenance, but construct only one lookup provider.

Equivalent root spellings therefore cannot create a module ambiguity. For
example, `modules`, `./modules`, and a symlink spelling of that same root
directory contribute one provider. The compiler retains the supplied
spellings and selects a deterministic display spelling for diagnostics, so
diagnostic output does not depend on root-option order.

File and directory symlinks encountered below a root are followed, including
links whose targets escape the canonical root directory. A module root is a
lookup mapping, not a filesystem sandbox. Its logical module path comes from
the lexical relative path visible below the configured root, not from the
canonical location of the symlink target:

```text
/modules/math/geometry.ska -> /shared/geometry.ska
```

still provides `math::geometry`. Broken or cyclic symlinks, unreadable
candidates, and candidates that do not resolve to regular files receive
diagnostics when reached.

All Skald names and module paths are case-sensitive. Resolution requires exact
component spelling even on a case-insensitive host filesystem:
`std::Str` does not resolve `std/str.ska`. If the host cannot represent or
distinguish configured candidates consistently by case, the loader diagnoses
the collision instead of selecting a platform-dependent winner.

The compiler retains canonical and display paths at different levels:

- each normalized root records its canonical directory for root equivalence
  and a deterministic user-facing display spelling (or set of spellings) for
  diagnostics;
- each loaded module records its logical `ModulePath`, supplying `ProviderId`,
  root-relative source path, user-facing display source path, and `SourceId`;
  and
- a candidate's canonical target path may be retained as optional diagnostic
  or I/O metadata, but is never its semantic identity or an ambiguity key.

There is no compilation-wide physical-file identity rule. Exact logical paths
collide by provider mapping, while distinct logical paths remain distinct
modules regardless of their physical targets or contents.

### Standard library

The standard library is an ordinary provider whose default location comes
from the compiler installation. One `--stdlib-root <directory>` replaces that
default provider, while `--no-stdlib` disables it. The two options are
mutually exclusive and neither renames `std`.

Standard modules use ordinary imports, visibility, loading, semantic phases,
and code generation. They are not automatically imported or eagerly compiled.
An ordinary `--module-root` may also provide modules below the `std` logical
prefix, but a duplicate exact module from the active standard-library provider
is an ambiguity rather than a shadowing override.

Future features may designate a declaration such as `std::Str` as a language
item. The responsible semantic phase should resolve the configured path once
to a declaration identity. Lowering and backends must not repeatedly compare
raw path strings.

## Proposed language surface

### Imports

The proposed grammar extension is:

```text
compilation-unit     = { import-declaration } { top-level-declaration } EOF
import-declaration   = module-import | selective-import
module-import        = "import" module-path ["as" identifier] ";"
selective-import     = "from" module-path "import" imported-declaration
                       {"," imported-declaration} ";"
imported-declaration = identifier ["as" identifier]
module-path          = identifier {"::" identifier}
```

Imports precede declarations, are module-wide, and have no source-order lookup
effect. Import lists do not accept a trailing comma initially.

Every `module-path` written in either import form is a canonical logical module
path. It is never resolved through another import's local alias. Import
declarations are therefore independent of their source order:

```ska
import std::Str as KesoStr;
from std::Str import Str;  // selects from canonical std::Str

// This would select a distinct canonical top-level module named KesoStr.
// It does not follow the alias above to std::Str.
from KesoStr import Other;
```

`::` is the module-path and module-qualification separator. It is
syntactically distinct from `.` for inline member access and `->` for access
through shared ownership:

```text
::   module path and module-qualified declaration
.    inline object member access
->   member access through shared ownership
```

This gives module bindings their own syntactically selected namespace. An
ordinary top-level or lexical binding with the same spelling does not shadow a
module binding:

```ska
import math::geometry as geometry;

fn area(shape: Shape) -> f64 {
    var geometry: Shape = shape;
    geometry.area();
    return geometry::circle_area();
}
```

The first `geometry` is an ordinary local selected by `.`. The second is the
module alias selected by `::`.

#### Module imports

A module import binds its complete path:

```ska
import app::model;
import math::geometry;

fn area() -> f64 {
    return math::geometry::circle_area();
}
```

It does not implicitly bind `geometry`. A one-segment alias may shorten the
local spelling:

```ska
import math::geometry as geometry;

fn area() -> f64 {
    return geometry::circle_area();
}
```

The alias grammar accepts exactly one identifier. A multi-segment alias such
as `as foo::geometry` is invalid initially.

An alias changes only spelling in its source module. It does not change the
target's canonical path, package ownership, declaration identity, diagnostics,
or spelling elsewhere.

A module import binds exactly one module:

- it does not import descendants;
- it does not inject declarations into unqualified lookup;
- it does not expose the target's own imports transitively; and
- it cannot be re-exported initially.

Every cross-module use must begin with a directly imported binding. Knowing an
absolute logical path does not bypass the import. Qualified names work in
every declaration context, including calls, types, construction, inheritance,
interface claims, and casts. Resolution replaces a successful spelling with
the selected dense declaration identity.

Plain paths may share prefixes, so importing `math::geometry` and
`math::trigonometry` is valid. An exact bound path may designate only one
module.

The same canonical module may have multiple local bindings:

```ska
import std::Str;
import std::Str as KesoStr;
import std::Str as OtherStr;

fn build() -> std::Str::Str {
    var first: KesoStr::Str = KesoStr::Str();
    var second: OtherStr::Str = OtherStr::Str();
    return std::Str::Str();
}
```

All bindings above resolve to one `ModuleId`. The source is loaded once and
the import graph contains one dependency edge regardless of the number of
bindings. Within the module-binding namespace, the resolver rejects only
binding conflicts:

- repeating the same local module binding;
- one local binding designating different modules; or
- two aliases with the same binding identifier.

Because `::` selects this namespace, a module alias may share spelling with an
ordinary top-level declaration, parameter, or local without ambiguity.

#### Selective declaration imports

A selective import explicitly introduces one or more public top-level
declarations into the importing module's ordinary top-level namespace:

```ska
from std::Str import Str, StrBuf;

fn build() -> Str {
    var buffer: StrBuf = StrBuf();
    return Str();
}
```

Each declaration may receive its own local alias:

```ska
from std::Str import Str, Str as HelloStr, StrBuf as HelloStrBuf;

fn build() -> HelloStr {
    var ordinary: Str = Str();
    var buffer: HelloStrBuf = HelloStrBuf();
    return HelloStr();
}
```

Without `as`, the declaration's source name is bound. With `as`, only the
alias is bound. The binding remains local to the importer and does not change
the declaration's canonical owner or semantic identity. For example,
`HelloStr` above still denotes the class canonically owned by
`std::Str::Str`.

Selective imports may name public classes, interfaces, defined functions, and
external functions. They cannot name private declarations, class members,
module bindings, or imports of the target module. A selective import makes its
source module reachable but does not also bind that module; a source that
needs both forms declares both imports.

The same canonical declaration may have multiple local bindings, whether they
are introduced in one declaration or several:

```ska
import std::Str;
from std::Str import Str;
from std::Str import Str as HelloStr;

fn build() -> std::Str::Str {
    var first: Str = Str();
    var second: HelloStr = HelloStr();
    return std::Str::Str();
}
```

`std::Str::Str`, `Str`, and `HelloStr` above all resolve to the same canonical
class identity. Aliasing does not create a new type or require a conversion.

Selective bindings participate in the existing ordinary top-level declaration
namespace. The resolver rejects:

- an imported name or alias colliding with a declaration in the importing
  module;
- two selective imports introducing the same local name, even when they
  select the same canonical declaration; and
- an unknown, private, or wrong-kind selected declaration.

Existing lexical rules continue to govern parameters and locals, which may
shadow an ordinary selectively imported declaration inside their scope.
Module bindings remain independently available through `::`.

Wildcard selection is not supported:

```ska
from std::Str import *; // invalid
```

This keeps the imported name set explicit. Adding another public declaration
to a dependency cannot create a new local collision in its consumers.

### Visibility

Top-level declarations are private to their defining module unless marked
`public`. The proposed grammar shape is:

```text
top-level-declaration = ["public"] (
    function-definition
  | external-function-declaration
  | class-declaration
  | interface-declaration
)
```

For example:

```ska
public class Counter {
}

public fn make_counter() -> Counter {
    return Counter();
}

fn normalize_seed(seed: i64) -> i64 {
    return seed;
}
```

An importer may use `Counter` and `make_counter` through its module binding or
an explicit selective import, but not `normalize_seed`. Qualification and
aliasing never bypass visibility.

The first boundary is only module-level:

- there is no package-private or library-private modifier;
- class-member visibility is unchanged;
- a public class exposes the member surface existing rules make accessible;
  and
- `public` controls Skald source access, not native linker visibility.

Modules sharing a textual path prefix do not gain access to each other's
private declarations.

### Per-module namespaces

Each module retains Skald's shared non-overloaded top-level declaration
namespace. Duplicate class, interface, defined-function, or external-function
names within one module remain errors. The same leaf name in different
modules is valid and denotes different declarations:

```text
app::model::Counter
test::model::Counter
```

Only explicitly selected declarations enter unqualified lookup. Implicit
unqualified lookup through a module import, wildcards, flattening, and
re-exports are deferred.

## Entry selection

### File and logical selectors

The CLI accepts exactly one of two entry selectors. A positional argument
selects a source file:

```text
skac /project/modules/app/main.ska --module-root /project/modules
```

The explicit `--entry <module-path>` option selects a logical module:

```text
skac --entry app::main --module-root /project/modules
```

The selectors are mutually exclusive. `--entry` may occur only once and its
argument must be a valid canonical `::`-separated module path. More than one
positional source, no selector, or both forms together is a usage error.

A logical selector resolves across all providers like an import. A file
selector keeps the canonical `.ska` suffix requirement and is normalized as
follows:

| File-entry location | Module identity |
|---|---|
| Inside exactly one applicable root | Derive its ordinary root-relative path. |
| Reached through multiple distinct normalized providers | Diagnose ambiguous entry identity, regardless of logical path, contents, or physical target. |
| Outside every root | Create a singleton provider using the valid file stem as a top-level module path. |

Selecting a rooted file and selecting its path must intern the same
`ModuleId`. A later import must not compile it twice.

Entry containment follows the same lexical root mapping as imports. A
canonical target reached through an in-root symlink does not cause an
otherwise outside positional path to inherit that root's logical identity.

### Root options

`--module-root <directory>` is repeatable and adds one anonymous filesystem
provider each time. Zero explicit module roots is valid: a positional entry
may become a singleton provider, and the active standard-library provider may
still satisfy imports.

Module-root options never create source-visible aliases and their order has no
lookup precedence. Canonically equivalent root spellings are coalesced before
provider construction. Overlapping but distinct roots remain distinct
providers: different logical paths may coexist, while an exact logical-path
collision is always ambiguous.

The standard-library choices are:

```text
# Use the compiler installation's default standard-library root.
skac --entry app::main --module-root /project/modules

# Replace that default with one explicit root.
skac --entry app::main --stdlib-root /sdk/modules

# Configure no automatic standard-library provider.
skac --entry app::main --no-stdlib
```

`--stdlib-root` may occur only once. Supplying it together with
`--no-stdlib` is a usage error.

### Default output names

`-o` or `--output` continues to override every default. Without an explicit
output, the final module-path component supplies the output base name.

For a positional file entry, the existing input-stem location is retained:

| Invocation | Executable | `--emit asm` |
|---|---|---|
| `skac app/main.ska` | `app/main` | `app/main.s` |

For a logical entry, there is no input location, so the final component is
placed in the current directory:

| Invocation | Executable | `--emit asm` |
|---|---|---|
| `skac --entry app::main` | `main` | `main.s` |
| `skac --entry tools::formatter` | `formatter` | `formatter.s` |

Entry selection does not affect the existing safe artifact-publication and
input/output-alias protections.

### Singleton file entry

Given:

```text
skac /tmp/my_main.ska
```

the compiler adds exactly:

```text
my_main -> /tmp/my_main.ska
```

It does not expose the directory:

```text
keso -> /tmp/keso.ska       # not provided
```

The singleton participates in ordinary resolution, so another module may
write `import my_main;`, subject to visibility and the acyclic-import rule. A
distinct rooted `my_main` is an ambiguity. The entry stem must be a valid
module component; an invalid stem receives a diagnostic instead of an
unspellable synthetic name.

Internally this is a singleton provider, not an implicit provider for `/tmp`.
It may have anonymous package provenance without changing its logical path.

### Entry function

Entry status belongs to the build, not to a special source module kind. Only
the selected module must contain:

```ska
fn main() -> i64 {
    return 0;
}
```

The entry `main` need not be public. A function named `main` in another
reachable module is ordinary and does not compete for entry status. The
backend exposes only the selected `FunctionId` through its host-ABI wrapper.

## Whole-program boundary

The entry plus its transitive imports is one program. An import makes its
module reachable even if no exported declaration is used. Unrelated files
below roots are ignored.

There is no separate library compilation, cached public interface, module
object file, or link-time Skald module resolution. Skald also has no top-level
executable state, so this design does not define module initialization order.

### Import cycles

The initial language rejects every import cycle, including self-imports. The
loader diagnoses a back edge with the complete cycle in import order:

```text
app::a imports app::b
app::b imports util::c
util::c imports app::a
```

This is a deliberate language restriction rather than an incidental recursive
loader failure. It keeps graph construction and declaration-surface
availability simple. A later language revision may allow declaration-only
cycles without changing the meaning of existing acyclic programs.

Because every reachable non-entry module has an import path from the entry, a
reachable module importing the entry creates a cycle and is rejected. The
singleton entry mapping remains a normal importable module identity; it simply
does not override the acyclic graph rule.

## Compiler design

### Compilation request and providers

The driver lowers CLI and future manifest inputs into explicit request state:

```text
CompilationRequest
  entry: FilePath | ModulePath
  providers:
    - zero or more filesystem roots
    - the default, overridden, or disabled standard-library provider
    - a singleton provider when required by a file entry
  target and artifact options
```

Provider order may be retained for diagnostics reproducing user input but has
no lookup precedence. A filesystem provider maps `a::b` to
`<root>/a/b.ska`. Discovery should probe requested paths rather than scan and
parse every source under every root.

### Identity and provenance

The compiler keeps these facts distinct:

```text
PackageId
  source ownership/provenance for a workspace, standard library, dependency,
  or singleton entry

ModulePath
  source-visible logical address used by imports

ModuleId
  dense compilation-local identity of one loaded module

SourceId
  concrete source instance and owner of source spans

ProviderId
  configured provider that supplied the source

CanonicalRootPath
  normalized directory identity used only to coalesce equivalent roots

DisplayRootPath
  deterministic user-facing root spelling used by diagnostics

RelativeSourcePath
  lexical path below the provider root that derives the ModulePath

DisplaySourcePath
  user-facing source spelling used in spans and diagnostics
```

The first CLI may assign anonymous request-local package identities because
package-private visibility does not exist yet. A future manifest can provide
stable identities or group roots without changing module paths.

Every declaration retains its owning `ModuleId`. Compilation-wide
`FunctionId`, `ClassId`, `InterfaceId`, member IDs, and other dense semantic
IDs remain the operational identities used by later phases.

Each module instance has its own `SourceId`. Reusing one physical file under
different logical paths therefore produces distinct `SourceId`, `ModuleId`,
AST, declaration, and nominal type identities. Canonical per-file paths or
filesystem identities may support byte caching or diagnostics, but do not
participate in semantic interning.

### Loading and phases

The proposed orchestration is:

```text
compilation request
  -> normalize providers and select the entry
  -> lex and parse the entry
  -> resolve imports and parse newly discovered sources until graph closure
  -> collect per-module declarations and public surfaces
  -> resolve bindings and declaration uses to dense identities
  -> type check one whole resolved program
  -> existing whole-program HIR, MIR, verification, backend, and emission
```

Loading owns root normalization, path mapping, I/O, source-instance creation,
provenance, and graph construction. It does not own semantic declaration
lookup. Lexing and parsing remain source phases with one product per module.
An unreadable or malformed import is a source failure, not an internal
compiler error.

`SourceDatabase` already supports multiple source files and source-tagged
spans. It remains request-local and becomes the source store for every loaded
module.

### Graph and resolution

The module graph records:

- one `ModuleId` and `SourceId` per loaded module;
- canonical `ModulePath`, package provenance, and provider;
- direct imports and their local bindings;
- the selected entry module; and
- deterministic order independent of discovery.

Resolution remains the only declaration-selection phase. One centralized
import-aware lookup service should:

- handle declarations local to the current module;
- recognize only direct imported bindings for qualified lookup;
- enforce public visibility and expected declaration kind;
- assign the existing dense declaration identity; and
- emit cross-file labels for private, ambiguous, and wrong-kind uses.

Type checking and lowering receive selected identities and never repeat
string-based import lookup. Skald needs no separate semantic linker:
resolution can produce flat whole-program tables with module ownership.

### Determinism, backend, and linkage

Discovery order, import order, and root option order must not affect identities
or output. A suitable allocation order is canonical module-path order, then
source declaration order, then existing member order. The entry is explicit
metadata, not a special allocation position.

Source-public declarations remain private compiler-generated native symbols.
Internal symbols continue to use collision-free dense identities rather than
lossy encodings of paths such as `a::b` and `a_b`. Only the selected entry
function feeds the ABI-visible `main` wrapper.

### External ABI declarations across modules

External declarations continue to request their exact foreign symbol. When
several reachable modules declare that symbol, the compiler coalesces
identical ABI assertions and rejects incompatible ones.

Two declarations are ABI-identical when they have:

- the same exact foreign symbol spelling;
- the same calling convention, which is fixed by the initial external
  boundary;
- the same parameter count and source-order parameter types; and
- the same result type.

Parameter names, module ownership, local import aliases, and `public`
visibility do not affect ABI identity. For example, declarations in different
modules may use different parameter names:

```ska
// In platform::first:
extern fn emit_value(value: i64) -> unit;

// In platform::second:
extern fn emit_value(number: i64) -> unit;
```

Both declarations retain their own module-owned `FunctionId`, visibility, and
source span. They resolve to one compilation-wide external-link identity, such
as an `ExternalSymbolId`, and calls through either declaration request the same
native `emit_value` symbol. Coalescing does not merge source declarations or
make a private declaration visible through another module.

An incompatible assertion is a source error before backend emission:

```ska
// In platform::first:
extern fn emit_value(value: i64) -> unit;

// In platform::second:
extern fn emit_value(value: f64) -> unit; // incompatible ABI assertion
```

The diagnostic labels every conflicting declaration and describes the
signature difference. Declaration and diagnostic ordering is deterministic
and independent of module discovery or import order.

A Skald function definition is never coalesced with an external declaration
because their leaf names match. Duplicate external declarations inside one
module also remain ordinary duplicate top-level declarations; coalescing
applies only after valid declarations from distinct modules have been
collected.

## Required diagnostic coverage

The implementation should diagnose at least:

- a missing, repeated, or conflicting positional/`--entry` selector;
- an invalid logical `--entry` spelling;
- repeated `--stdlib-root` or conflicting `--stdlib-root` and
  `--no-stdlib`;
- invalid root, standard-library, entry, or module-component paths;
- an entry file with multiple possible logical identities;
- a missing imported or logical entry module;
- an exact module supplied by multiple distinct normalized providers,
  regardless of candidate contents or physical identity;
- a module-path case mismatch or host case collision;
- a broken or cyclic symlink, unreadable candidate, or candidate that does not
  resolve to a regular file;
- repeated or conflicting local module bindings, selective-import binding
  conflicts, alias-based import-source mistakes, multi-segment module aliases,
  and invalid wildcard imports;
- private cross-module access or use without a direct import;
- a qualified declaration of the wrong kind;
- incompatible cross-module declarations of one exact external symbol, with
  every conflicting signature labeled;
- unreadable or malformed imported source;
- an invalid or missing entry `main`; and
- self-imports and import cycles, including the complete cycle chain.

Ambiguity and visibility diagnostics should label the importing use and every
relevant candidate or private declaration across source files.

## Complete example

Given:

```text
/project/modules/app/main.ska
/project/modules/app/model.ska
/project/modules/math/trigonometry.ska
/deps/modules/math/geometry.ska
/sdk/modules/std/io.ska
```

the logical tree is:

```text
app::main
app::model
math::trigonometry
math::geometry
std::io
```

`app/main.ska` may contain:

```ska
from app::model import origin;
import math::geometry as geometry;
from std::io import print_i64;

fn main() -> i64 {
    print_i64(geometry::measure(origin()));
    return 0;
}
```

and build with:

```text
skac --entry app::main \
    --module-root /project/modules \
    --module-root /deps/modules \
    --stdlib-root /sdk/modules
```

Moving dependency or SDK directories changes only build configuration. Adding
another `math/geometry.ska` makes the build ambiguous instead of shadowing a
candidate.

## Initial exclusions

The first version excludes:

- source-declared module or package names;
- command-line aliases that rename module trees;
- multi-segment module aliases or resolving import sources through local module
  aliases;
- relative, parent, wildcard, or re-export imports;
- implicit unqualified lookup through module imports;
- namespace flattening and facade construction;
- package-private visibility and new member visibility;
- directory modules and implicit index files;
- top-level state and module initialization;
- manifests, version solving, registries, and lockfiles;
- separate compilation, serialized interfaces, and binary Skald libraries;
- native export of source-public declarations; and
- eager compilation of every source below a root.

## Promotion result

The completed decisions were promoted into the living language and compiler
contracts, followed by a coverage reconciliation that restored the loader,
resolution, identity, ABI, and diagnostic obligations needed for
implementation. This record preserves the reasoning that led to them and must
not be used as a parallel specification. An implementation roadmap should
consume the complete frozen contracts rather than restate or reopen them.
