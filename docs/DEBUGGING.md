# Compiler Debugging Artifacts

Every major compiler product is inspectable and deterministic. These renderers
are public Rust APIs so phase tests, temporary debugging tools, and future CLI
inspection options all use the same representation:

| Product | Renderer |
|---|---|
| Tokens | `lexer::dump_tokens` |
| Source AST | `syntax::dump_ast` |
| Resolved program | `resolve::dump_resolved` |
| Typed HIR | `hir::dump_hir` |
| MIR | `mir::dump_mir` |
| GNU assembly | `backend::emit_assembly`, or `skac --emit asm` |

Each earlier-phase renderer is covered by an exact unit-test snapshot beside its implementation. Run a focused dump test with, for example:

```text
cargo test -p skald-compiler token_dump -- --nocapture
cargo test -p skald-compiler ast_dump -- --nocapture
cargo test -p skald-compiler resolved_dump -- --nocapture
cargo test -p skald-compiler hir_dump -- --nocapture
cargo test -p skald-compiler mir_dump -- --nocapture
```

The snapshots normally assert rather than print the artifact. While debugging a phase, its public renderer can be called from the colocated test and printed with `eprintln!`; `--nocapture` then exposes it without adding a second serialization path.

Token, AST, resolved, HIR, and MIR renderers share only the private quoting,
escaping, indentation, and span-suffix primitives in `dump_format`. Each phase
retains ownership of its dump structure and vocabulary, so changing one IR
does not create a cross-phase serialization abstraction.

Resolved, HIR, and MIR dumps render callable declarations separately from
local definitions. This makes a bodyless external declaration visible without
inventing an empty body, and keeps signature/linkage inspection independent of
executable control flow. MIR calls display stable function IDs rather than
backend symbols.

Calls and returns make payload presence explicit in MIR dumps. An `i64` call
defines a value and an `i64` return names one; a `unit` call and return omit the
value entirely. This makes accidental fictitious unit values visible at the
phase boundary and enforceable by the verifier.

MIR dumps render `goto` targets and boolean branches with their condition,
true target, and false target. Blocks remain in dense `BlockId` order rather
than traversal order, so loops, joins, and unreachable blocks have stable,
inspectable output before backend lowering exists.

Assembly uses matching deterministic function-and-block labels of the form
`.Lska_fn_N_block_M`, plus one `.Lska_fn_N_epilogue` label. Blocks are emitted
in `BlockId` order, not traversal order. This makes forward jumps, back edges,
diamonds, joins, and all return paths directly comparable with the MIR dump.

Conditional lowering preserves `IfArm`, ordered `ElifArm` entries, and an
optional `ElseArm` in the AST, resolved, and HIR dumps. Its MIR dump exposes the
corresponding false continuation chain and join directly, making evaluation
order and omitted unreachable joins inspectable without reading assembly.

Nested native conditional goldens exercise these representations and rely on
the golden runner's two independent compiler processes for every successful
assembly and every failure diagnostic. Determinism is therefore an externally
checked compiler property rather than an assumption from unit dumps.

That guarantee covers every implemented numeric family. Typed HIR
renders `f64` constants as `F64 0xHHHHHHHHHHHHHHHH`, and MIR renders them as
`const.f64 0xHHHHHHHHHHHHHHHH`; neither dump asks the host library to format a
decimal float. The primitive golden corpus exercises independent compiler
processes for `u64`, `u8`, `f64`, mixed integer/SSE signatures, malformed
spellings, overflow, and exact-type failures.

Assembly is directly available through the public compiler command:

```text
cargo run -p skac -- input.ska --emit asm -o build/input.s
```

The golden runner compiles every successful program to assembly twice in separate `skac` processes and compares the bytes. It likewise compiles every failure twice and compares exact stderr snapshots. This catches nondeterminism in IDs, ordering, paths, labels, formatting, and diagnostics at the externally visible boundary. Native cases then compare stdout bytes and process status independently. In particular, `tests/golden/run/println_i64.ska` exercises exact-symbol external call lowering and the runtime output ABI while its `.stdout` and nonzero `.exit` sidecars keep the two observations separate. `tests/golden/run/conditionals.ska` uses output-producing condition functions to expose left-to-right testing, skipped later conditions, selected-arm-only execution, `else`, and fallthrough without `else`.

MIR verification runs:

1. as a debug assertion immediately after HIR-to-MIR lowering;
2. unconditionally in the explicit target-independent MIR pass pipeline;
3. again at the backend trust boundary before target legality and instruction selection.

The repetition is intentional. The first check identifies lowering defects near their source, the pass-pipeline check protects future transformations, and the backend check prevents invalid library-created MIR from being miscompiled.
