# Boundaries for the Next Language Slice

Status: post-O6 extension contract.

The first vertical slice is complete. The next slice may add language behavior, but it should extend the following boundaries instead of bypassing or merging them.

## Stable responsibilities

1. **Source and diagnostics** own files, UTF-8 byte spans, line maps, structured diagnostics, and stable rendering. New phases reuse these types rather than inventing phase-local locations or printing errors directly.
2. **Lexing** owns spelling and token formation only. It does not decide names, types, or constant semantics.
3. **Syntax** owns grammatical source shape and recovery. AST nodes may grow, but they remain unresolved and preserve source spans.
4. **Resolution** is the only source-name-to-declaration selection phase. Every executable reference below it uses a typed stable ID. Callable declarations own signatures and linkage independently of optional local definitions.
5. **Typed HIR** owns language-level types and selected semantic operations. It preserves the callable declaration/definition split, and a successful HIR contains no unresolved calls, untyped expressions, or placeholder error nodes.
6. **MIR** owns executable evaluation order, storage, temporaries, calls with optional results, basic blocks, and terminators without target registers or ABI rules. Calls consult canonical declarations rather than definition storage.
7. **The MIR pass pipeline** is the visible home for target-independent verification and transformations. Correctness does not depend on an optimization pass.
8. **Backends** own target legality, ABI lowering, frame and register decisions, target instructions, entry wrappers, symbols, and assembly formatting. Unsupported valid MIR is rejected explicitly until implemented.
9. **The driver/toolchain layer** owns file I/O, phase orchestration, artifact publication, subprocesses, and process exit codes. Compiler phases never invoke host tools.
10. **The C runtime** exposes only its versioned ABI. It is not a place for language semantics that can live in generated code or the future standard library.

These are responsibility boundaries, not promises that every current Rust data structure is frozen. Types and APIs should evolve when new semantics require it, while preserving dependency direction.

## Extension checklist

For each new construct or type:

1. update the draft specification or explicitly record the provisional behavior;
2. extend the lexical and grammar contract where syntax changes;
3. add source AST and recovery behavior;
4. assign or reuse stable resolved identities without adding name lookup below resolution;
5. make types and selected operations explicit in HIR;
6. lower evaluation and control flow explicitly into MIR;
7. extend MIR verification before relying on the new representation;
8. make each backend either lower the new MIR or reject it through target legality;
9. add focused phase tests, deterministic dump coverage, compile-failure goldens, and successful execution goldens where observable;
10. update architecture and roadmap status in the same change.

## Likely next-slice pressure points

The `bool` and Niflheim-style `if` / `elif` / `else` slice is complete through
C6 in [`BOOL_CONDITIONALS_ROADMAP.md`](BOOL_CONDITIONALS_ROADMAP.md). It adds a
semantic boolean type in HIR and MIR, multiple MIR blocks with explicit
conditional and unconditional terminators, control-flow-aware MIR verification,
branch selection in the backend, and exact native and failure coverage. The
implementation preserves the phase boundary: branches are
ordinary MIR control flow and the backend never rediscovers high-level syntax.

The selected next slice adds the remaining primitive types `u64`, `u8`, and
`f64`, following [`PRIMITIVE_TYPES_ROADMAP.md`](PRIMITIVE_TYPES_ROADMAP.md).
It deliberately separates full-width unsigned arithmetic, narrow-value
canonicalization, and floating-point/SSE ABI work into distinct milestones.
T0 through T3 are complete: the contracts are fixed, runtime ABI version 4 can
observe all three types directly, numeric spellings share one classified
pipeline, and `u64` works end-to-end with modular arithmetic and integer-class
ABI lowering. T4's narrow `u8` representation and canonicalization are next.
The other candidates below describe separate architectural pressures and
should receive their own scoped roadmap.

Adding the first inline object instead stresses layout, construction state, assignment, cleanup order, receiver access, and return conventions. That slice needs a written ABI/layout contract before code generation and should not be combined casually with shared ownership or exceptions.

Broader integer operations primarily stress specified edge-case semantics and instruction selection. AArch64 stresses the target interface and should leave semantic phases unchanged.

Arrays, optionals, loops/iterators, checked exceptions, shared ownership, and general local reference aliases remain deferred. Each crosses several of the boundaries above and should receive its own scoped roadmap rather than entering as an incidental parser feature.

The completed output and boolean/conditional slices intentionally do not
generalize foreign linkage or I/O. T0 extends their exact-symbol C-ABI contract
only with by-value `u64`, `u8`, and `f64` as their implementation milestones
land. Alternate link names, variadic calls, non-primitive ABI types, ownership-bearing arguments,
cross-module declaration coalescing, recoverable output errors, and the final
standard-library I/O interface remain deferred and require explicit contracts
before implementation. `unit` remains a payload-free result type and is not
yet permitted as a parameter, local, or first-class value.

## Deliberately replaceable implementation choices

The stack-heavy x86-64 location strategy, textual GNU assembly syntax, one-library-crate organization, recursive-descent parser, non-SSA MIR, and source-tree runtime discovery are practical stage-0 choices. They may be replaced behind their current boundaries when measurements or new features justify the work. The next slice should not preemptively replace them without a concrete requirement.
