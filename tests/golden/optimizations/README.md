# Optimization Goldens

This group exercises selectable final-MIR optimization profiles across the
complete source-to-native boundary. The focused fixtures combine local scalar
simplification, proof-aware CFG retention, static startup and shutdown,
function values, and whole-world definition retention. Each fixture runs with
the default profile, the exact `none` reference profile, every individually
disabled local pass, both earlier canary passes disabled in turn, and all
registered passes disabled together.

The checked-integer fixtures separately cover successful quotient, remainder,
and shift protocols for every primitive integer type, exact signed and width
boundaries, dynamic and effectful exclusions, nested expressions, static
lifecycle, and ownership destruction. Their failure matrix preserves operand
order and exact panic observations across default, `none`, checked-folding-
disabled, CFG-cleanup-disabled, and all-pass-disabled products.

Use `make golden-filter GOLDEN_FILTER='optimizations/**'` for the ordinary
focused suite. Use `scripts/golden.sh --determinism full --filter
'optimizations/**'` to repeat both compiler and native processes.
