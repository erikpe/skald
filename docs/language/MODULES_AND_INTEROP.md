# Modules and Foreign Interoperation

Status: authoritative for the implemented initial module system and
foreign-function boundary. [Feature maturity](STATUS.md) summarizes compiler
support, and the [implemented grammar](GRAMMAR.md) owns accepted syntax.

## Compilation units and modules

One compiler invocation selects an entry module and compiles its complete
reachable module graph as one program. Each module is one UTF-8 `.ska` source
mapping and has a logical identity selected by roots or positional-entry
rules; source files contain no `module` declaration. The source-text
convenience API constructs one in-memory singleton module and therefore has no
filesystem context in which to resolve imports.

The implemented top-level declaration kinds are:

- function definitions;
- bodyless external-function declarations; and
- class and interface declarations.

Declarations are collected before callable bodies are resolved, so a body may
refer to a later top-level declaration in the same file. There is no
source-order overload set or fallback to another file.

## Top-level namespace

Classes, interfaces, defined functions, and external functions share one
non-overloaded top-level namespace. A name may occur there at most once,
including across declaration kinds. In particular:

- two identical external declarations in one module are a duplicate rather
  than a coalesced declaration;
- an external declaration and a function definition cannot share a name; and
- classes, interfaces, and functions cannot share names with one another.

Class-member and lexical-local namespaces are separate and are defined by
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#members-and-namespaces) and
[functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#lexical-scopes-and-locals).
Skald has no top-level overloading or import precedence. Qualification and
visibility follow the module rules below.

## Program entry point

The selected entry module must define exactly this source function:

```ska
fn main() -> i64 {
    return 0;
}
```

The entry function has no parameters, returns `i64`, and has a Skald body. A
missing `main`, a different parameter or result signature, and an external
declaration named `main` are compile-time errors. How a target exposes this
source function to its host process is an implementation concern.

The implemented [process-argument contract](PROCESS.md) preserves this
signature. Programs obtain the host invocation vector through an ordinary,
explicitly imported `std::process::args()` call rather than entry parameters.

## External-function declarations

The implemented external form is a top-level declaration with no Skald body:

```ska
extern fn read_value(seed: i64) -> i64;
extern fn emit(value: i64) -> unit;
```

Every parameter is passed by value and must have type `i64`, `u64`, `u8`,
`f64`, or `bool`. The result may be any of those types or `unit`. Parameter
names are required by the grammar. Calls use the ordinary exact-type, arity,
and left-to-right evaluation rules from
[functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#calls-and-results).

The source identifier is also the exact external symbol requested from the
selected target's linker. There is no source form for a different link name or
calling convention. External declarations have no compiler-supplied body, and
the compiler does not recognize runtime operations by their spelling.

An external declaration is a trusted interoperability assertion. Skald checks
uses against the declared source signature, but it cannot prove that a linked
definition uses compatible foreign types or behavior. A missing definition is
reported when executable linkage is attempted. An incompatible definition is
outside Skald's safety and behavior guarantees.

The repository runtime's current operations are ordinary declarations under
this same rule; they are not language built-ins. Their C signatures, output
records, and version contract remain implementation documentation rather than
additional language semantics.

## Unsupported external forms

The external boundary rejects or has no syntax for:

- alias, object, array, optional, shared, interface, and function-value
  parameters or results;
- ownership or lifetime transfer across the boundary;
- variadic parameters;
- source-selected calling conventions or link names;
- external variables, static data, classes, methods, and lifecycle members;
  and
- repeated declarations within one module intended to describe one foreign
  definition.

Some type shapes above can be parsed in a general declaration position but
are rejected semantically for external functions. Their appearance in the
grammar does not extend the interoperability contract.

The frozen
[optional-values contract](OPTIONAL_VALUES.md#declaration-and-abi-boundary)
retains this rejection for every optional parameter and result. It does not
define a foreign optional representation or ownership convention.

The [shared-ownership profile](SHARED_OWNERSHIP.md#exclusions)
deliberately preserves this restriction: shared values do not cross the
external boundary, and the allocator functions remain compiler/runtime
ABI operations rather than source-visible shared-handle interoperation.

## Intrinsic-function declarations

The compiler accepts a bodyless top-level intrinsic declaration:

```ska
import std::str;

public intrinsic fn panic(message: std::str::Str) -> unit;
```

`intrinsic` is contextual in this declaration position. An intrinsic
declaration otherwise has ordinary top-level visibility, name, parameter,
result, module, import, qualification, duplicate, and source-span behavior.
It has neither a Skald body nor an external link symbol. It is not an external
declaration and does not widen the trusted foreign ABI.

The canonical standard-library declaration above resolves to the exact
identity and signature
`std::error::panic(message: std::str::Str) -> unit`. The `std::error` module
therefore has an ordinary explicit dependency on `std::str`; it does not rely
on string-literal reachability or a hidden type alias. `panic` remains
private-by-default under the ordinary rule, so the canonical declaration must
spell `public`. Source uses reach the same declaration identity through direct
qualification, a module import, or a selective import. There is no automatic
prelude binding.

The canonical `std::str` implementation selectively imports `panic` for its
bounds failures, so the installed `std::error` and `std::str` modules form an
ordinary two-module cycle. Both directions retain their exact source import
and declaration identities; neither direction is compiler-special
reachability or a re-export.

The implemented intrinsic registry contains only closed canonical
standard-library identities. The compiler validates each declaration while
module ownership, visibility, parameter modes, exact type identities, result
type, body absence, and source spans are still available. It rejects an
intrinsic at any other module path, an unrecognized intrinsic name, a
body-bearing declaration, a signature or visibility that differs from its
registry entry, and any attempt to combine the declaration with ordinary
function, member, lifecycle, interface, override, or external-function
syntax. For `std::error::panic` specifically, the required signature is one
public, by-value, exact `std::str::Str` parameter and a `unit` result. An
unused valid declaration remains bodyless metadata through executable IR. A
panic call statement resolves to the stable intrinsic function identity and
becomes the dedicated non-returning panic operation before HIR.
Expression-position use is rejected because panic does not produce a value.
Intrinsics cannot be entry points, methods,
initializers, lifecycle members, interface requirements, overrides, or native
exports.

After validation, resolved and typed compiler phases retain a stable intrinsic
identity rather than matching the source spelling `panic`. The source
semantics of invoking the canonical identity are owned by the
[frozen panic design](ERRORS.md#frozen-panic-design). No intrinsic is
authorized merely because the parser accepts this declaration form.

The implemented [standard I/O compiler contract](../compiler/IO.md) adds five
private canonical declarations under `std::io`, using primitive and
whole-array alias parameters. The canonical `std::f64` module additionally
declares private `_to_bits(f64) -> u64` and `_from_bits(u64) -> f64`
intrinsics behind ordinary public `to_bits` and `from_bits` wrappers. Their
calls become pure bit-reinterpretation operations before executable IR and
never enter the foreign or runtime ABI. Panic, the two binary64 bit
operations, and the five I/O operations form one closed
registry keyed by exact module path and declaration name. Calls become
dedicated typed operations before executable IR; the declarations do not
widen the restricted external-function ABI or create prelude bindings.

## Initial module system

The initial module system is implemented for whole-program compilation. This
section defines its source-visible contract; the
[module-system compiler contract](../compiler/MODULE_SYSTEM.md) owns roots,
filesystem resolution, CLI entry selection, loading, identities, and
determinism.

### Modules and paths

One module is one logical compilation instance of a `.ska` source mapping.
There is no source `module` declaration. A module root derives the canonical
logical path from the source's relative path:

```text
<root>/app/main.ska        -> app::main
<root>/math/geometry.ska   -> math::geometry
<root>/std/collections.ska -> std::collections
```

The suffix is omitted. Every directory component and the file stem must be a
valid Skald identifier. Module paths are non-empty and case-sensitive.
Directories provide path prefixes only; they are not modules or namespaces
with declarations. A module and a descendant may coexist:

```text
<root>/math.ska          -> math
<root>/math/geometry.ska -> math::geometry
```

Module roots are anonymous lookup locations. Their command-line spelling is
never part of an import. Roots compose one logical tree without precedence,
and a shared prefix conveys neither ownership nor visibility:

```text
/project/modules/math/trigonometry.ska -> math::trigonometry
/deps/modules/math/geometry.ska        -> math::geometry
```

The standard library participates through the same rules. Nothing is
implicitly imported, and `std` has no language-level privilege. A future
string-literal contract may designate a path such as `std::Str` through an
explicit language-item rule without making its module otherwise special.

### Import syntax

Imports precede top-level declarations and are visible throughout their
module:

```text
compilation-unit     = { import-declaration } { top-level-declaration } EOF
import-declaration   = module-import | selective-import
module-import        = "import" module-path ["as" identifier] ";"
selective-import     = "from" module-path "import" imported-declaration
                       { "," imported-declaration } ";"
imported-declaration = identifier ["as" identifier]
module-path          = identifier { "::" identifier }
```

Import lists have no trailing comma. `import`, `from`, `as`, and `public` are
contextual words recognized only in the exact forms above and in the
visibility form below; they remain ordinary identifiers elsewhere. `::` is
reserved for module paths and module-qualified declaration use; `.` remains
inline member access and `->` remains shared-owner member access.

Every import source is a canonical logical module path. An earlier local alias
cannot be used as another import's source:

```ska
import std::Str as KesoStr;
from std::Str import Str; // valid
from KesoStr import Other; // names canonical top-level module KesoStr
```

Imports are therefore independent of source order.

### Module imports

A module import binds its complete canonical path:

```ska
import math::geometry;

fn area() -> f64 {
    return math::geometry::circle_area();
}
```

It does not bind only the final component. A one-identifier alias may shorten
the local spelling:

```ska
import math::geometry as geometry;

fn area() -> f64 {
    return geometry::circle_area();
}
```

Module aliases are exactly one identifier; `as foo::geometry` is invalid.
An alias changes only local spelling, not canonical path, provenance,
visibility, or declaration identity. Diagnostics may reproduce the alias at
the importing use, but identify the target by its canonical module and
declaration; the alias never renames the target in another module.

A module import:

- binds exactly one module, not its descendants;
- adds no declaration to unqualified lookup;
- does not expose the imported module's imports;
- does not re-export anything; and
- permits qualified use only through a directly imported binding.

Knowing an absolute logical path does not bypass the direct-import
requirement. A qualified declaration may appear anywhere its unqualified form
could appear, including types, calls, construction, inheritance, interface
claims, and casts. A qualified public class may select a static method with
`module_binding::Class.method(arguments)`; qualification establishes class
visibility but does not bypass member privacy.

The implemented [static-field contract](STATIC_FIELDS.md) reuses this
same boundary for `module_binding::Class.name`. A derived-class spelling
retains the declaring static field's canonical identity, and multiple module
bindings never create additional storage. Public static fields can be selected
across modules; private ones remain accessible only from callables owned by
their declaring class, including in cyclic module graphs.

The same canonical module may have multiple local bindings:

```ska
import std::Str;
import std::Str as KesoStr;
import std::Str as OtherStr;
```

All three bindings select one `ModuleId`; the source is loaded and parsed once
and contributes one dependency edge. The resolver rejects only:

- repetition of the same local module path;
- one local module binding designating different modules; or
- two aliases using the same binding identifier.

Module bindings occupy a namespace selected syntactically by `::`. A module
alias may therefore share spelling with a top-level declaration, parameter,
or local:

```ska
import math::geometry as geometry;

fn area(shape: Shape) -> f64 {
    var geometry: Shape = shape;
    geometry.area();
    return geometry::circle_area();
}
```

### Selective imports

A selective import introduces named public top-level declarations into the
importing module's ordinary top-level namespace:

```ska
from std::Str import Str, StrBuf;
from std::Str import Str as HelloStr, StrBuf as HelloStrBuf;
```

Without `as`, the declaration's source name is bound. With `as`, only the
alias is bound. Selective imports may name public classes, interfaces,
defined functions, and external functions. They cannot name private
declarations, members, module bindings, or declarations merely imported by
the target module.

Selecting declarations makes the source module reachable but does not bind
the module itself. Code needing both forms declares both:

```ska
import std::Str;
from std::Str import Str;
from std::Str import Str as HelloStr;
```

Multiple local names may select the same canonical declaration. They do not
create new types or functions, require conversions, or change the
declaration's canonical diagnostic owner. Each local ordinary name must
nevertheless be unique. The resolver rejects:

- a selective name or alias colliding with a declaration in the importing
  module;
- two selective imports introducing the same local name, even when they
  select the same canonical declaration; and
- an unknown, private, or unsupported declaration kind.

Existing lexical scopes may shadow a selectively imported ordinary name.

Wildcard imports are invalid:

```ska
from std::Str import *; // invalid
```

There is no implicit unqualified lookup, namespace flattening, or re-export.
Adding a public declaration to a dependency therefore cannot introduce a new
local binding or collision in an importer.

### Visibility and namespaces

Accessible imported top-level functions and qualified static methods may form
capture-free function values. Reference formation uses the ordinary module and
declaring-class visibility check once; a validly formed private static value
may then cross public internal parameter and result boundaries without
repeating member-name access. External and intrinsic declarations never form
function values, and no function value crosses the primitive-only foreign ABI.

Top-level declarations are private to their defining module unless marked
`public`:

```text
top-level-declaration = ["public"] (
    function-definition
  | external-function-declaration
  | class-declaration
  | interface-declaration
)
```

Qualification and aliases never bypass top-level privacy. Top-level
visibility and class-member visibility are independent:

- there is no package-private or library-private visibility;
- shared path prefixes grant no access;
- a public class may contain private fields, instance methods, and static
  methods, and neither same-module access nor an import grants access to them;
  the exact rule is
  [declaring-class privacy](CLASSES_AND_LIFECYCLE.md#declaring-class-privacy);
  and
- `public` controls Skald source access, not native symbol export.

Each module retains one non-overloaded ordinary top-level declaration
namespace. Duplicate leaf names within a module remain errors; equal leaf
names in different modules denote distinct declarations.

### Reachability and cycles

Compilation is whole-program only. The selected entry module and the
transitive closure of its imports form the program. Unused imports still make
their modules reachable. Multiple bindings or selective imports from the same
module contribute one reachability edge. Unrelated source files are not
compiled.

Two or more modules may import each other directly or through a longer cycle.
The compiler loads the complete reachable graph, collects all top-level
declarations and public surfaces, and then resolves imports and uses across
the whole program. A cycle does not create transitive bindings or re-exports:
each qualified or selective use still requires its own exact direct import.

A direct self-import is invalid because it adds no reachability and creates a
redundant qualified path to declarations already owned by the current module.
Compiler-owned dependencies, such as a string literal in `std::str`, may point
back to their owning module because they create no source binding.

Modules have no executable top-level state and an import cycle therefore has
no initialization order. Any future top-level initialization feature must
define cyclic initialization independently or reject it. Cyclic imports do
not permit inheritance cycles, recursive inline containment, or any other
semantic structure rejected by its owning language rule.

### Selected entry module

The build selects one ordinary module as the entry. Only that module must
define:

```ska
fn main() -> i64 {
    return 0;
}
```

Its `main` need not be public. Functions named `main` in other reachable
modules are ordinary functions and do not compete for entry status. Only the
selected function is exposed through the host entry wrapper.

The entry may be selected by logical path or file path and need not live below
a configured root. Those build rules, including singleton file entries and
default output names, are defined by the
[compiler contract](../compiler/MODULE_SYSTEM.md#entry-selection-and-command-line).

### External declarations across modules

An external declaration remains a module-owned trusted ABI assertion.
Valid declarations in different modules for the same exact foreign symbol
coalesce only when their ABI signatures are identical: equal calling
convention, parameter count and source-order parameter types, and result type.
Parameter names, module ownership, import aliases, and visibility do not
affect ABI identity.

Incompatible declarations are a source error and every conflicting
declaration is labeled. Coalescing does not merge source declarations or
visibility. Repeated external declarations within one module remain ordinary
duplicate top-level declarations. A Skald function definition never coalesces
with an external declaration sharing its leaf name.

### Initial exclusions

The frozen initial system has no:

- source-declared module or package names;
- relative or parent imports;
- wildcard imports, re-exports, implicit import flattening, or facade
  construction;
- multi-segment module aliases or alias-based import sources;
- package-private visibility;
- directory modules or implicit index files;
- manifests, registries, versions, or dependency distribution;
- separate compilation, serialized interfaces, or binary Skald libraries;
- top-level state or module initialization; or
- native export of source-public declarations.

The top-level-state exclusion does not preclude the separately implemented
class-owned [static-field profile](STATIC_FIELDS.md). That profile adds neither
module-owned storage nor module initialization, and source-public static fields
do not become native exports.

Package identities may later group roots and define package-private access
without changing existing logical paths or imports. Separate compilation may
later consume the same frozen module graph and declaration identities without
changing whole-program source meaning.

## Broader interoperation

Broader foreign interoperation must separately settle foreign type mappings,
ownership, callbacks, variadics, alternate symbols and calling conventions,
failure behavior, and which guarantees can cross the trust boundary. None of
those choices should be inferred from the current primitive-only profile. The
current foreign-failure boundary is defined in
[errors and exceptional control flow](ERRORS.md#current-runtime-failures).

## Implementation boundary

The exact external source symbol is part of the implemented language contract
because it is the only available link-name selection. Target ABI
classification, C type widths, registers, stack placement, and
compiler-generated symbol handling are implementation details owned by the
[backend and target contract](../compiler/BACKEND.md). Runtime symbols and link
markers are owned by the [runtime ABI](../compiler/RUNTIME_ABI.md). Tool
invocation and artifact publication are owned by
[driver and artifacts](../compiler/DRIVER_AND_ARTIFACTS.md).
