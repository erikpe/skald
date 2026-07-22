# Modules and Foreign Interoperation

Status: authoritative for the implemented compilation-unit, top-level
namespace, entry-point, and source-visible external-function contracts.
[Feature maturity](STATUS.md) remains authoritative for compiler support, and
the [implemented grammar](GRAMMAR.md) owns exact source shape.

## Current compilation unit

One compiler invocation accepts one UTF-8 source file. The command-line
compiler requires the canonical `.ska` suffix. That file is the complete
compilation unit: it has no declared module identity and cannot import another
Skald file.

The implemented top-level declaration kinds are:

- function definitions;
- bodyless external-function declarations; and
- class declarations.

Declarations are collected before callable bodies are resolved, so a body may
refer to a later top-level function or class in the same file. There is no
source-order overload set or fallback to another file.

## Top-level namespace

Classes, defined functions, and external functions share one non-overloaded
top-level namespace. A name may occur there at most once, including across
declaration kinds. In particular:

- two identical external declarations are a duplicate rather than a
  coalesced declaration;
- an external declaration and a function definition cannot share a name; and
- a class and function cannot share a name.

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

## Future modules and broader interoperation

Modules and multiple-file programs are an **open question**, not reserved or
exploratory syntax. Skald currently defines no `module`, `import`, `export`,
package, qualification, or visibility form. Examples inherited from Niflheim
or older Skald drafts are not a compatibility promise.

A future module design must settle at least:

- source paths and canonical module identity;
- imports, qualification, re-exports, and ambiguity handling;
- public and private visibility;
- dependency cycles and deterministic initialization order;
- packages, build inputs, and separate-compilation artifacts; and
- whether compatible external declarations may coalesce across units.

Broader foreign interoperation must separately settle foreign type mappings,
ownership, callbacks, variadics, alternate symbols and calling conventions,
failure behavior, and which guarantees can cross the trust boundary. None of
those choices should be inferred from the current primitive-only profile. The
current foreign-failure boundary is defined in
[errors and exceptional control flow](ERRORS.md#current-runtime-failures).

## Implementation boundary

The exact external source symbol is part of the implemented language contract
because it is the only available link-name selection. Target ABI
classification, C type widths, registers, stack placement, compiler-generated
symbol spelling, runtime link markers, tool invocation, and artifact
publication are implementation details. During the documentation migration,
they remain owned by the existing
[backend](../REPO_STRUCTURE.md#x86-64-system-v-backend),
[runtime](../REPO_STRUCTURE.md#runtime), and
[driver](../REPO_STRUCTURE.md#driver-and-artifacts) sections until their
focused documents replace those authorities.
