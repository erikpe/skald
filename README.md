# Skald

Skald is an exploratory, statically typed, compiled programming language intended for learning, small personal projects, and compiler experimentation. Its design aims to remain understandable enough that one person can study and implement the compiler and runtime without giving up deterministic resource management or object-oriented programming.

The compiler is named **`skac`**, and the canonical suffix for Skald source files is **`.ska`**.

## Design overview

Skald is built around deterministic lifetimes rather than garbage collection:

- class types are inline values by default;
- `shared T` is a non-null, reference-counted owning handle to a heap allocation;
- `ref name: T` and `mut ref name: T` are call-scoped alias-binding modes rather than general reference types;
- caller-owned borrow anchors keep aliased storage alive without general lifetime inference or Rust-style borrow checking;
- assignment updates an existing value without ending its lifetime;
- `init`, `assign`, and `destroy` are contextual lifecycle declarations, and destruction is deterministic even through polymorphic shared handles;
- optionality is explicit with `T?`, and ordinary non-optional values are never null.

The object model includes classes, single inheritance, interfaces, explicit virtual dispatch, and distinct receiver access modes. Ordinary instance `fn` methods have read-only receivers, while `mut fn` methods may mutate their receivers. Read-only access and `final` fields are shallow across shared ownership.

The initial design deliberately excludes garbage collection, raw pointers in safe code, general-purpose lifetime analysis, concurrency, captured closures, and user-defined generics. Checked exceptions are part of the intended language because exceptional control flow must preserve deterministic cleanup, although they may be implemented after the initial non-exception core.

## Initial implementation

The stage-0 `skac` compiler is written in Rust and organized as an explicit modern compiler pipeline. The initial host and target platform is Linux x86-64 using the System V ABI. `skac` emits textual assembly, which is assembled and linked with a minimal C runtime using the system toolchain.

The compiler is designed for multiple backends. AArch64 Linux is expected after the x86-64 path has established the target-independent IR and backend boundary.

Niflheim remains a frequent source of design and testing experience, while Skald deliberately uses a cleaner phase architecture rather than reusing the organically grown Python implementation.

## Status

Skald is currently an exploratory language design. Milestones M0 through M8 are implemented, completing the first vertical slice: the stage-0 compiler accepts the documented subset, emits deterministic x86-64 System V assembly, links it with the minimal runtime, produces native executables, and has source-to-process and exact compile-failure golden coverage. Later language features remain future work. The language specification remains a draft, and syntax, semantics, and implementation interfaces may change as further slices are implemented and tested.

## Development

Initial development requires Linux, a stable Rust toolchain with rustfmt and Clippy, GNU Make, a C11 compiler, and an archiver. The repository has no third-party Rust dependencies at M0.

Common commands:

```text
make fmt            # format Rust source
make check          # formatting, type checks, Clippy, Rust tests, and C runtime tests
make build-check    # type-check every Rust workspace target
make compiler-test  # Rust workspace tests only
make golden-test    # native source-to-executable golden cases
make runtime        # build build/runtime/libskald_runtime.a
make runtime-test   # build and run direct C runtime tests
cargo run -p skac -- --help
```

Build artifacts are written below `target/` and `build/`.

Compile an executable or stop after deterministic textual assembly emission:

```text
make runtime
cargo run -p skac -- samples/vertical/exit_42.ska -o build/exit_42
cargo run -p skac -- samples/vertical/exit_42.ska --emit asm -o build/exit_42.s
```

Executable output uses `cc` by default and links `build/runtime/libskald_runtime.a`. Set `CC` to select another compatible C compiler driver or `SKALD_RUNTIME_ARCHIVE` to use another runtime archive. Without `-o`, executable output uses the input path without `.ska`; assembly output uses `.s`.

## History

Skald began as a draft called **Niflheim2**, using the earlier Niflheim language and compiler as a starting point. Niflheim used garbage-collected reference objects. The experimental successor introduced inline object values, deterministic destruction, reference-counted shared ownership, and call-scoped borrowing.

Those changes eventually made the design a new language rather than a compatible Niflheim revision, so it was renamed Skald and moved into its own repository. The old implementation remains useful as historical context and as a record of compiler-design lessons, but it is not the implementation base or normative specification for Skald.

In the current development checkout, the Niflheim repository is available as the sibling directory [`../niflheim`](../niflheim).

## Documentation

- [Skald draft language specification](docs/SKALD_DRAFT_SPEC.md) — the canonical description of the language design.
- [Repository structure and compiler architecture](docs/REPO_STRUCTURE.md) — design principles, phase boundaries, backend layout, runtime boundary, and testing structure.
- [First vertical slice roadmap](docs/FIRST_VERTICAL_SLICE_ROADMAP.md) — the minimal end-to-end implementation plan and completion criteria.
- [Compiler debugging artifacts](docs/DEBUGGING.md) — deterministic phase dumps, assembly inspection, and verifier boundaries.
- [Next-slice boundaries](docs/NEXT_SLICE_BOUNDARIES.md) — responsibilities and extension rules that future language work should preserve.
- [Niflheim language specification](../niflheim/docs/LANGUAGE_MVP_SPEC_V0.1.md) — historical background for the language from which the first Skald draft was derived.

Skald documentation takes precedence whenever its behavior differs from Niflheim.
