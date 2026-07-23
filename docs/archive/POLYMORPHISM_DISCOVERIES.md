# Polymorphism Maintainability Discoveries

Status: resolved and archived.

This record preserves the final maintainability follow-up identified during
polymorphism hardening.

## Resolved discovery

### Separate dynamic-call targeting from ABI marshaling

- [x] Kept `backend/x86_64_sysv/lower/call.rs` as the call-selection facade.
- [x] Moved direct, virtual, and interface target selection into the private
      `call/target.rs` owner.
- [x] Moved incoming and outgoing argument/result marshaling into the private
      `call/marshal.rs` owner.
- [x] Preserved `CallLayout`, instruction order, stack alignment, object-origin
      forwarding, result normalization, and structured backend errors.
