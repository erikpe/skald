---
name: structure-skald-rust-modules
description: Maintain facade-oriented recursive Rust module organization in Skald. Use whenever implementing, extending, moving, reviewing, or refactoring Rust code in the Skald repository, especially when working with `mod.rs`, adding submodules, exposing public APIs, introducing substantial implementation logic, or adding unit and integration tests.
---

# Structure Skald Rust Modules

Keep Skald's recursive `mod.rs` layout while making module boundaries descriptive, stable, and easy to navigate.

## Organize the module

Treat `mod.rs` as the module's front door. Prefer placing these items there:

- module-level `//!` documentation;
- private `mod` declarations;
- intentional `pub mod` declarations when the nested namespace is itself public;
- explicit `pub use` statements defining the module's public facade;
- small public entry functions that coordinate submodules;
- small central API types when no more cohesive submodule owns them;
- `#[cfg(test)] mod tests;` when unit tests live in `tests.rs`.

Move substantial implementation concerns into descriptively named files. Split by responsibility, not by individual type or function. Typical compiler responsibilities include `model.rs`, `scanner.rs`, `parser.rs`, `lower.rs`, `verify.rs`, `dump.rs`, `emit.rs`, and `tests.rs`.

Do not impose a zero-logic rule on `mod.rs`. Keep small cohesive modules together when splitting would only add navigation overhead. Extract code when `mod.rs` starts mixing distinct responsibilities, contains a substantial algorithm or private state machine, or becomes difficult to scan as a facade.

## Control visibility

Keep submodules private by default:

```rust
mod scanner;
mod token;

pub use scanner::{lex, LexOutput};
pub use token::{Token, TokenKind};
```

Prefer selective re-exports over `pub use module::*`. Use `pub mod` only when callers should deliberately name that submodule. Preserve the public path through the facade so internal files can later move without changing downstream code.

Keep implementation-only imports, helpers, state, and algorithms in the submodule that owns them. Do not make an item public merely to let a sibling module or test access it; use the narrowest suitable Rust visibility.

## Place tests

Keep a few short, tightly local unit tests inline when they do not obscure the implementation. Move a substantial test block to `tests.rs` beside the module:

```text
lexer/
├── mod.rs
├── scanner.rs
├── token.rs
├── dump.rs
└── tests.rs
```

Declare it from `mod.rs`:

```rust
#[cfg(test)]
mod tests;
```

The external unit-test module may use `super::*` and retains access to private items through Rust's module privacy rules. Put tests that require only the public crate API in the crate's `tests/` directory. Keep complete source-to-diagnostic or source-to-executable behavior in Skald's top-level golden suite.

## Apply the pattern during changes

1. Inspect the affected module before editing and identify its public facade and implementation responsibilities.
2. Add new code to the cohesive owning file; create a submodule when the new concern is substantial or independently understandable.
3. Keep or restore `mod.rs` as a concise facade when the touched area is already oversized.
4. Avoid unrelated repository-wide rearrangement. Refactor the affected module when needed to prevent new work from worsening its structure.
5. Update imports and re-exports without widening the public API accidentally.
6. Keep documentation and tests with the responsibility they describe.
7. Run `cargo fmt` and proportionate focused tests. Run the repository's full Rust checks when the change spans module boundaries or public APIs.

## Review checklist

Before finishing Rust work in Skald, confirm:

- `mod.rs` communicates the module API and structure quickly;
- substantial algorithms and private state have descriptive homes;
- submodules are private unless their namespace is intentionally public;
- re-exports are explicit and minimal;
- files are divided by cohesive responsibility rather than arbitrary size alone;
- large unit-test collections do not dominate implementation files;
- integration and golden tests use the appropriate repository-level locations;
- the refactor did not create needless tiny files or unrelated churn.
