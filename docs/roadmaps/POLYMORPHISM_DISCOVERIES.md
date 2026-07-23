# Polymorphism Maintainability Discoveries

Status: actionable follow-ups found during final polymorphism hardening.

These items are outside the completed language-feature scope. They preserve
evidence and a reviewable boundary for later maintainability work.

## High priority

### Split callable-body resolution by responsibility

- **Problem:** `resolve/resolver/body.rs` is about 1,000 lines and combines
  lexical scopes, statements, base initialization, narrowing, expressions,
  direct and member calls, assignment, and binding diagnostics.
- **Evidence:** call and member selection alone occupy roughly a third of the
  file, while statement and binding orchestration form separate cohesive
  responsibilities. The existing `body/place.rs` extraction demonstrates the
  intended private-submodule pattern.
- **Owner:** `crates/skald-compiler/src/resolve/resolver/body.rs`.
- **Boundary:** retain `resolve_callable_body` as the facade; extract call/member
  resolution and statement/binding resolution into private `body/` modules
  without changing resolved IR, diagnostic order, recovery, or dump output.

## Medium priority

### Separate dynamic-call targeting from ABI marshaling

- **Problem:** `backend/x86_64_sysv/lower/call.rs` combines direct, virtual, and
  interface target selection with argument-area planning, object-origin
  forwarding, result normalization, and individual argument stores.
- **Evidence:** the file is over 500 lines and shared ownership will add more
  receiver/alias lifetime mechanics to this boundary.
- **Owner:** `crates/skald-compiler/src/backend/x86_64_sysv/lower/call.rs`.
- **Boundary:** keep one call-selection entry point, but move target selection
  and argument/result marshaling into cohesive private owners while preserving
  the current `CallLayout`, instruction order, stack alignment, and structured
  backend errors.
