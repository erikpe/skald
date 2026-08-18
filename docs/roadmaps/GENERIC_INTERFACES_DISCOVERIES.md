# Generic Interfaces Discoveries

Status: pending follow-up after the generic interfaces roadmap.

## Canonical structural template keys

- **Problem:** Definition-time duplicate interface-bound detection currently
  needs a local recursive comparison that ignores source spans. Later
  application caching will need the same semantic equality and hashing rule.
- **Evidence:** `generic_templates/interface_resolution.rs` contains
  `same_type`, while `ResolvedTemplateType` deliberately derives equality that
  includes diagnostic spans and therefore cannot be used as a canonical key.
- **Likely owner:** Shared generic-template structural IR and the cross-kind
  specialization coordinator.
- **Priority:** Medium; address when application keys become first-class.
- **Boundary:** Introduce one span-free canonical key or shared structural
  comparison/hash facade, migrate bound deduplication and caches to it, and
  retain source spans only in provenance records.

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
