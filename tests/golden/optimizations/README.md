# Optimization Goldens

This group exercises selectable final-MIR optimization profiles across the
complete source-to-native boundary. The focused fixtures combine local scalar
simplification, proof-aware CFG retention, static startup and shutdown,
function values, and whole-world definition retention. Each fixture runs with
the default profile, the exact `none` reference profile, every individually
disabled local pass, both earlier canary passes disabled in turn, and all
registered passes disabled together.

Use `make golden-filter GOLDEN_FILTER='optimizations/**'` for the ordinary
focused suite. Use `scripts/golden.sh --determinism full --filter
'optimizations/**'` to repeat both compiler and native processes.
