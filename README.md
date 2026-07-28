# Skald

Skald is an exploratory, statically typed, compiled language for learning,
small personal projects, and compiler experimentation. It combines an
object-oriented source model with inline values and deterministic resource
management while keeping the compiler and runtime understandable.

The compiler is named `skac`, and Skald source files use the `.ska` suffix.

## Current compiler

The current implementation accepts one UTF-8 source file and supports
primitive values, functions, lexical control flow, exact nominal inline
classes, deterministic copying and destruction, owning class parameters and
results, non-null reference-counted shared objects, owning shared fields,
hidden borrow anchors, call-scoped object views, single inheritance,
declaring-class private members, receiverless static methods,
virtual/interface dispatch, type tests, and checked object casts. The
compiler also supports plain checked object casts in receiver, alias-argument,
field, inline copy-construction, value-parameter, result, slicing, and
whole-object assignment contexts, plus allocation-free owner-preserving shared
casts and explicit exact-class shared copy allocation. A `shared T` expression
is an owner handle: `*owner` selects its bounded non-owning pointee place and
`owner->member` crosses exactly one shared edge. Raw shared handles are not
implicitly treated as inline objects. The
[language status matrix](docs/language/STATUS.md) is the authoritative support
summary; the [implemented grammar](docs/language/GRAMMAR.md) defines the exact
accepted syntax.

Skald currently targets Linux x86-64 using the System V ABI. The compiler can
emit GNU assembler text using Intel syntax with `noprefix`, or link a native
executable against the repository's small C runtime. `x86_64-sysv` is the only
registered target; additional targets are future work rather than current
compatibility promises.

## Build and use

Install the prerequisites in the [development workflow](docs/development/README.md),
then from the repository root run:

```text
make runtime
cargo run --locked -p skac -- samples/inline_counter.ska -o build/inline_counter
```

Emit assembly without linking:

```text
cargo run --locked -p skac -- samples/inline_counter.ska --emit asm -o build/inline_counter.s
```

`skac --help` is the exact command-line reference. Run `make help` for the
repository command inventory and `make check` for the ordinary validation
gate. CLI, toolchain, runtime selection, and artifact guarantees are defined
by [Driver and Artifacts](docs/compiler/DRIVER_AND_ARTIFACTS.md).

## Documentation

Start at the [documentation index](docs/README.md). Principal references are:

- [language overview](docs/language/README.md),
  [status](docs/language/STATUS.md),
  [implemented grammar](docs/language/GRAMMAR.md), and frozen
  [module-system](docs/language/MODULES_AND_INTEROP.md) and
  [string](docs/language/STRINGS.md) language contracts;
- [compiler architecture](docs/compiler/README.md),
  [phases and IR](docs/compiler/PHASES_AND_IR.md),
  [module-system contract](docs/compiler/MODULE_SYSTEM.md),
  [backend](docs/compiler/BACKEND.md), and
  [runtime ABI](docs/compiler/RUNTIME_ABI.md);
- [development workflow](docs/development/README.md),
  [testing](docs/development/TESTING.md), and
  [debugging](docs/development/DEBUGGING.md); and
- [active roadmaps](docs/roadmaps/README.md) and
  [archived implementation roadmaps](docs/archive/README.md).

Current feature direction and unresolved design belong in the status matrix
and focused language documents. Implementation order and dependencies belong
in active roadmaps.

## History

Skald began as a draft called Niflheim2, derived from the earlier Niflheim
language. Its shift from garbage-collected reference objects to inline values,
deterministic destruction, explicit ownership, and call-scoped aliases made it
a distinct language and repository.

Niflheim remains useful historical context and a source of compiler-design
lessons, but it is neither Skald's implementation base nor its normative
specification. In this checkout it is available at [`../niflheim`](../niflheim).
