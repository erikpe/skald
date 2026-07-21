# Future Development Boundaries

This document records the architectural constraints and unresolved design work
that future Skald slices should preserve. It describes the current extension
surface; completed implementation history lives in [`archive/`](archive/README.md).

## Stable compiler responsibilities

1. **Source and diagnostics** own files, UTF-8 byte spans, line maps,
   structured diagnostics, and stable rendering.
2. **Lexing** owns token formation and source spelling, not names or types.
3. **Syntax** owns grammatical source shape and recovery. AST nodes remain
   unresolved.
4. **Resolution** is the only source-name selection phase. Later phases use
   stable typed identities.
5. **Typed HIR** owns language types, receiver access, selected operations, and
   selected callable/member identities.
6. **MIR** owns executable evaluation order, addressable places, transient
   values, construction, calls, basic blocks, and terminators without target
   registers or byte offsets.
7. **The MIR pass pipeline** owns target-independent verification and future
   transformations. Correctness must not depend on optimization.
8. **Backends** own target legality, data layout, ABI classification, frames,
   registers, symbols, instruction selection, and assembly formatting.
9. **The driver** owns phase orchestration, file I/O, tool invocation, artifact
   publication, and process exit codes.
10. **The C runtime** exposes a small versioned ABI. Language facilities that
    can live safely in generated code or the future standard library should not
    migrate into it.

These are responsibility boundaries, not promises that individual Rust data
structures are frozen.

## Rules for extending the language

Every substantial feature should:

1. state its source and runtime semantics before implementation;
2. update the grammar or explicitly record that no syntax changes;
3. assign stable identities during resolution rather than performing name
   lookup below it;
4. make types, access modes, and selected operations explicit in HIR;
5. express evaluation and control flow explicitly in MIR;
6. extend MIR verification before relying on a new representation;
7. keep ABI, layout, and register decisions out of target-independent IR;
8. make each backend either support the new MIR or reject it structurally;
9. add focused phase tests, deterministic dumps, failure diagnostics, and
   native goldens where behavior is observable;
10. update living documentation and place the completed implementation plan in
    `docs/archive/`.

## Object-model sequence

The implemented object core deliberately stops at direct local inline objects
with primitive fields. The safest progression is:

1. **Alias parameters.** Add `ref` and `mut ref` parameters over existing
   places. Keep aliases call-scoped and non-storable so local references remain
   cheap and no general borrow checker is required. The focused implementation
   plan is the [Alias Parameters Roadmap](ALIAS_PARAMETERS_ROADMAP.md).
2. **Inline object fields.** Extend place projection and layout dependency
   handling before introducing cleanup. Recursive by-value containment must be
   rejected.
3. **Destruction.** Add `destroy`, initialized-place state, reverse-order scope
   cleanup, and cleanup-aware control-flow edges.
4. **Object value semantics.** Add copy construction and assignment before
   object parameters/results, return storage, temporaries, and permitted
   elision.
5. **Polymorphism.** Add inheritance, base projections, virtual dispatch,
   interfaces, casts, and dynamic type metadata.
6. **Shared ownership.** Add allocation, reference counting, complete dynamic
   destruction, and syntax-directed borrow anchors.
7. **Checked exceptions.** Integrate partial construction and cleanup with
   exceptional control flow rather than retrofitting it afterward.

Each step needs a dedicated roadmap. In particular, object parameters or
results should not precede their copy, destruction, ABI, and return-storage
contracts.

## Other planned language work

- loops and iterator protocols;
- arrays and their storage/initialization rules;
- optional values and scoped access to conditional payloads;
- integer division, remainder, comparisons, bitwise operations, shifts, and
  explicit casts;
- richer floating-point operations and user-facing formatting;
- strings and a Skald-written standard library;
- access control, `final`, static members, and broader module organization.

Arrays, optionals, loops, aliases, destruction, shared ownership, and checked
exceptions all interact with lifetime or control-flow rules. They should not be
added as isolated parser features.

## Compiler evolution

- Add Linux AArch64 behind the existing backend boundary.
- Introduce SSA only when concrete optimization work justifies it; conversion
  should be an explicit pass or a replaceable IR boundary.
- Replace the stack-heavy x86-64 location strategy with register allocation
  without changing MIR semantics.
- Add multiple source files, modules, and incremental compilation only after
  ownership of declarations and compilation sessions is specified.
- Keep deterministic artifacts and structured verifier errors as these systems
  become more sophisticated.

Alternate link names, variadic calls, object-bearing FFI, cross-module
declaration coalescing, concurrency, captured closures, user-defined generics,
and package management remain outside the current plan.
