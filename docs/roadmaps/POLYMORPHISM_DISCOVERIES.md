# Polymorphism Maintainability Discoveries

Status: one actionable follow-up remains from final polymorphism hardening.

These items are outside the completed language-feature scope. They preserve
evidence and a reviewable boundary for later maintainability work.

## Remaining discovery

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
