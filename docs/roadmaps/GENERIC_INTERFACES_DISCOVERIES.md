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

## Separate syntax closing from source request discovery

- **Problem:** Generic specialization request collection has two distinct
  traversal responsibilities in one implementation module: closing syntax
  types under a substitution and discovering application requests throughout
  source declarations and expressions.
- **Evidence:** `specialization/requests.rs` contains the `SyntaxTypeCloser`
  implementation and its diagnostics followed by the independent
  `SourceRequestScanner` AST traversal; the module is roughly 700 lines and
  changes to either traversal require navigating the other.
- **Likely owner:** The specialization request facade, with private
  `syntax_type_closer` and `source_request_scanner` implementation modules.
- **Priority:** Low; the responsibilities are internally coherent and no
  correctness or testability defect is known.
- **Boundary:** Move the two private implementations without changing the
  public request API, diagnostic ordering, traversal order, or specialization
  coordinator. Do this after the roadmap so I10 remains a hardening change
  rather than a large structural rewrite.
