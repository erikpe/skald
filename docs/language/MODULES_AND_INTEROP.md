# Modules and Foreign Interoperation

Status: authoritative for the implemented single-file compilation unit and
foreign-function boundary, and for the frozen initial module-system language
contract. [Feature maturity](STATUS.md) remains authoritative for compiler
support, and the [implemented grammar](GRAMMAR.md) owns syntax currently
accepted by the compiler.

## Current compilation unit

One compiler invocation accepts one UTF-8 source file. The command-line
compiler requires the canonical `.ska` suffix. That file is the complete
compilation unit: it has no declared module identity and cannot import another
Skald file.

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

- two identical external declarations are a duplicate rather than a
  coalesced declaration;
- an external declaration and a function definition cannot share a name; and
- classes, interfaces, and functions cannot share names with one another.

Class-member and lexical-local namespaces are separate and are defined by
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#members-and-namespaces) and
[functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#lexical-scopes-and-locals).
Skald has no top-level overloading, import precedence, qualification, or
visibility rule in the implemented language.

## Program entry point

An executable program must define exactly this source function:

```ska
fn main() -> i64 {
    return 0;
}
```

The entry function has no parameters, returns `i64`, and has a Skald body. A
missing `main`, a different parameter or result signature, and an external
declaration named `main` are compile-time errors. How a target exposes this
source function to its host process is an implementation concern.

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

The current external boundary rejects or has no syntax for:

- alias, object, array, optional, shared, interface, and function-value
  parameters or results;
- ownership or lifetime transfer across the boundary;
- variadic parameters;
- source-selected calling conventions or link names;
- external variables, static data, classes, methods, and lifecycle members;
  and
- repeated declarations intended to describe one foreign definition.

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

## Frozen initial module system

The initial module system is a **frozen design** but is not implemented.
The compiler therefore still rejects the syntax in this section and accepts
only the single-file form above. This section fixes the source-visible
contract for implementation; the
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
claims, and casts.

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

Qualification and aliases never bypass privacy. The initial boundary is only
module-level:

- there is no package-private or library-private visibility;
- shared path prefixes grant no access;
- class-member visibility is unchanged, so a public class or interface exposes
  only the member surface allowed by the existing member rules; and
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

Every import cycle is invalid, including self-import. A diagnostic reports the
complete cycle in import order. A module importing the selected entry through
the reachable graph therefore creates a cycle and is rejected.

This rule is source semantics, not an incidental recursive-loader limitation.
There is no module initialization order because the initial language has no
top-level executable state.

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
- package-private visibility or new member visibility;
- directory modules or implicit index files;
- manifests, registries, versions, or dependency distribution;
- separate compilation, serialized interfaces, or binary Skald libraries;
- top-level state or module initialization; or
- native export of source-public declarations.

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
