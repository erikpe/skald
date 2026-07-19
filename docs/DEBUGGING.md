# Compiler Debugging Artifacts

The first vertical slice keeps every major compiler product inspectable and deterministic. These renderers are public Rust APIs so phase tests, temporary debugging tools, and future CLI inspection options all use the same representation:

| Product | Renderer | Established by |
|---|---|---|
| Tokens | `lexer::dump_tokens` | M1 |
| Source AST | `syntax::dump_ast` | M2 |
| Resolved program | `resolve::dump_resolved` | M3 |
| Typed HIR | `hir::dump_hir` | M4 |
| MIR | `mir::dump_mir` | M5 |
| GNU assembly | `backend::emit_assembly`, or `skac --emit asm` | M6/M7 |

Each earlier-phase renderer is covered by an exact unit-test snapshot beside its implementation. Run a focused dump test with, for example:

```text
cargo test -p skald-compiler token_dump -- --nocapture
cargo test -p skald-compiler ast_dump -- --nocapture
cargo test -p skald-compiler resolved_dump -- --nocapture
cargo test -p skald-compiler hir_dump -- --nocapture
cargo test -p skald-compiler mir_dump -- --nocapture
```

The snapshots normally assert rather than print the artifact. While debugging a phase, its public renderer can be called from the colocated test and printed with `eprintln!`; `--nocapture` then exposes it without adding a second serialization path.

Assembly is directly available through the public compiler command:

```text
cargo run -p skac -- input.ska --emit asm -o build/input.s
```

The golden runner compiles every successful program to assembly twice in separate `skac` processes and compares the bytes. It likewise compiles every failure twice and compares exact stderr snapshots. This catches nondeterminism in IDs, ordering, paths, labels, formatting, and diagnostics at the externally visible boundary.

MIR verification runs:

1. as a debug assertion immediately after HIR-to-MIR lowering;
2. unconditionally in the explicit target-independent MIR pass pipeline;
3. again at the backend trust boundary before target legality and instruction selection.

The repetition is intentional. The first check identifies lowering defects near their source, the pass-pipeline check protects future transformations, and the backend check prevents invalid library-created MIR from being miscompiled.
