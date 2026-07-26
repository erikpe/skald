# Optional-Values Maintainability Discoveries

Status: actionable follow-up after completion of the optional-values roadmap.

The optional implementation is centralized behind the existing syntax,
type-checking, HIR, MIR, verifier, and backend facades. The focused audit found
one larger internal reorganization worth scheduling separately rather than
mixing a high-churn mechanical move into feature hardening.

## Split optional MIR verification by analysis responsibility

**Priority:** Medium.

**Evidence:** `crates/skald-compiler/src/mir/verify/optional.rs` is about 1,400
lines and owns three independently understandable responsibilities:
instruction-local optional structure and failure-edge checks, path-sensitive
definite optional-storage initialization, and path-sensitive checked-view
guard analysis. The file is cohesive at the feature level, but navigating or
changing one analysis currently requires scanning the other two.

**Likely owner:** `crates/skald-compiler/src/mir/verify/optional/`.

**Useful boundary:** Keep a small private `optional/mod.rs` facade with the
existing `Verifier` entry points. Move structural checks, initialized-storage
analysis, and guard analysis into private responsibility-named submodules.
Preserve verifier invocation order, diagnostic order and wording, MIR public
paths, and the existing focused mutation corpus. Do not combine this move with
semantic changes to optional state or guards.
