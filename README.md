# Skald

Skald is an exploratory, statically typed, compiled language for learning,
small personal projects, and compiler experimentation. It aims to keep both the
language and its implementation understandable without giving up deterministic
resource management or an object-oriented programming model.

The compiler is named **`skac`**. Skald source files use the **`.ska`** suffix.

## Language direction

Skald is designed around deterministic lifetimes rather than garbage
collection:

- class types are inline values by default;
- `shared T` is planned as a non-null reference-counted owning handle;
- `ref name: T` and `mut ref name: T` are call-scoped alias bindings; the
  current restricted profile supports exact inline class places, while later
  shared sources will require caller-owned anchors;
- assignment updates an existing value without ending its lifetime;
- `init`, `assign`, and `destroy` are contextual lifecycle declarations;
- optionality is explicit with `T?`; ordinary non-optional values are not null.

The broader design includes classes, single inheritance, interfaces, explicit
virtual dispatch, receiver mutability, and deterministic destruction. The
current implementation intentionally supports a smaller subset described
below. The language specification remains a draft and may change as future
slices exercise the design.

## Implemented language

The current Linux x86-64 compiler supports:

- one UTF-8 source file with ASCII identifiers and `//` comments;
- `i64`, `u64`, `u8`, `f64`, `bool`, and payload-free `unit` results;
- literals, unary numeric negation, and exact-type `+`, `-`, and `*`;
- local variables, nested lexical blocks, functions, recursion, and direct
  calls;
- `if` / `elif` / `else` with exact boolean conditions;
- restricted exact-symbol `extern fn` declarations over primitive values;
- inline classes with primitive executable fields, one explicit initializer,
  direct local construction, field reads/writes, and statically dispatched
  receiver methods;
- class-typed field declarations with nominal resolution, source-level
  rejection of recursive inline containment, target-independent nested
  object-place paths, direct field construction, initializer liveness, and
  executable projected reads, writes, method receivers, and alias arguments;
- read-only `fn` and mutable `mut fn` receiver access;
- restricted call-scoped `ref` and `mut ref` class parameters over inline
  locals, method receivers, and forwarded aliases;
- optional contextual `destroy { ... }` members and automatic deterministic
  cleanup of owning inline locals on normal block and `return` exits, including
  recursive class fields in reverse declaration order;
- exact-class copy constructors and copy assignments, with user-defined or
  recursively synthesized field behavior for local and projected destinations;
- internal exact-class value parameters whose caller-constructed copies are
  owned and cleaned once by the callee;
- internal exact-class function and method results through explicit
  caller-owned return storage;
- bounded owning object temporaries with reverse full-expression cleanup and
  deterministic direct-initialization/return constructor elision;
- deterministic left-to-right operand and argument evaluation;
- textual x86-64 System V assembly, native linking, exact diagnostics, and a
  small C runtime with primitive output functions.

Owning inline objects may cross an internal call boundary as exact-class value
arguments copied from existing or produced sources, and may return from
internal functions or methods through explicit caller-owned storage. Produced
sources are materialized and cleaned at their full-expression boundary unless
an ungrouped exact-class constructor is eligible for direct local or return
construction. Inheritance, interfaces, `shared`, arrays, optionals, loops, and
checked exceptions are not implemented yet. Object-bearing external
signatures remain unsupported.

Restricted alias parameters compile through syntax, typed HIR, verified MIR,
and the internal x86-64 pointer ABI without copying object bytes. Native and
compile-failure goldens cover access, forwarding, overlap, `self`, initializer
aliases, evaluation order, and mixed register/stack signatures.

See [the grammar notes](grammar/README.md) for the exact accepted source subset
and [the draft specification](docs/SKALD_DRAFT_SPEC.md) for the broader language
design.

## Compiler design

The stage-0 compiler is written in Rust and follows an explicit pipeline:

```text
source → tokens → AST → resolved IR → typed HIR → verified MIR
       → x86-64 backend → GNU assembly → system linker + C runtime
```

Semantic phases use stable identities rather than repeating source-name lookup.
MIR is target-independent and verified before backend lowering. Target layout,
ABI classification, frame planning, instruction selection, and assembly syntax
remain inside the backend. The compiler is structured to admit additional
backends and a later SSA-based optimization layer without changing the source
or semantic phases.

Skald currently targets Linux x86-64 System V. Linux AArch64 is the next
expected backend after the language core grows further.

## Building and using `skac`

Development requires Linux, stable Rust with rustfmt and Clippy, GNU Make, a
C11 compiler, and an archiver. The Rust workspace has no third-party crate
dependencies.

```text
make check          # formatting, checks, Clippy, Rust/golden tests, runtime tests
make golden-test    # source-to-native and compile-failure golden cases
make runtime        # build build/runtime/libskald_runtime.a
make runtime-test   # direct C runtime tests

cargo run -p skac -- samples/inline_counter.ska -o build/inline_counter
cargo run -p skac -- samples/inline_counter.ska --emit asm -o build/inline_counter.s
```

Executable output uses `cc` by default. `CC` selects another compatible compiler
driver, and `SKALD_RUNTIME_ARCHIVE` selects another runtime archive. Output is
published atomically; failed compilation or linking preserves an existing
destination.

## Future work

The next language slices should deepen object semantics rather than broaden the
syntax indiscriminately. Likely directions are:

1. hardening and publishing the restricted object-value profile;
2. inheritance, interfaces, virtual dispatch, and casts;
3. `shared` ownership and borrow anchors;
4. loops/iterators, arrays, optionals, and checked exceptions;
5. an AArch64 backend and, when useful, SSA conversion and optimization.

These are directions, not promises of syntax or ordering. Each substantial
feature should receive a focused design and implementation plan before work
begins. Current extension constraints are collected in
[Future Development Boundaries](docs/NEXT_SLICE_BOUNDARIES.md).

## History

Skald began as a draft called **Niflheim2**, derived from the earlier Niflheim
language. Moving from garbage-collected reference objects to inline values,
deterministic destruction, shared ownership, and call-scoped aliases changed
the design enough to make it a separate language and repository.

Niflheim remains useful historical context and a source of compiler-design
lessons, but it is neither Skald's implementation base nor its normative
specification. In this checkout it is available at [`../niflheim`](../niflheim).

## Documentation

- [Documentation index](docs/README.md)
- [Draft language specification](docs/SKALD_DRAFT_SPEC.md)
- [Implemented grammar and semantic subset](grammar/README.md)
- [Repository structure and compiler architecture](docs/REPO_STRUCTURE.md)
- [Future development boundaries](docs/NEXT_SLICE_BOUNDARIES.md)
- [Compiler debugging artifacts](docs/DEBUGGING.md)
- [Archived implementation roadmaps](docs/archive/README.md)

Skald documentation takes precedence wherever it differs from Niflheim.
