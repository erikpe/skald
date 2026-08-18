# Generic Interfaces Discoveries

Status: pending follow-up after the generic interfaces roadmap.

## Shared contextual capability predicates

- **Problem:** Definition-independent generic-interface validation must reason
  about structural terms before ordinary compound types have been interned,
  while ordinary capability predicates currently accept closed HIR or resolved
  identities. The small structural predicates can drift from their ordinary
  owners as the type system evolves.
- **Evidence:** `generic_templates/interface_resolution.rs` mirrors the closed
  value-parameter, result, alias, optional, array, and shared-target categories
  used by type checking.
- **Likely owner:** A target-independent type-capability facade above resolution
  and type checking.
- **Priority:** Medium; no current behavior mismatch is known.
- **Boundary:** Extract category-based predicates that both structural template
  validation and ordinary closed-type validation adapt to, without making
  resolution depend on HIR or type checking.
