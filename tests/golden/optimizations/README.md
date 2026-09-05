# Optimization Goldens

This group exercises selectable final-MIR optimization profiles across the
complete source-to-native boundary. The focused fixtures combine local scalar
simplification, proof-aware CFG retention, static startup and shutdown,
function values, and whole-world definition retention. Each fixture runs with
the default profile, the exact `none` reference profile, every individually
disabled local pass, both CFG cleanup passes disabled in turn, and all
registered passes disabled together.

`proof_provenance_normalization.ska` is the focused two-stage boundary matrix.
It combines nested logical conditions, optional/array/shared path-sensitive
uses, static initialization and shutdown, and ownership destruction. Its
companion panic fixture pins the selected failure span and runtime trace across
default, `none`, post-proof-canary-disabled, reachability-disabled, and
all-pass-disabled variants.

`post_proof_cfg_shape.ska` is shared by the focused source-lowering test and
the golden matrix, pinning a transitive forwarding chain and instruction-
bearing merge. `post_proof_cfg_canonicalization.ska` broadens the native
matrix across methods, static startup and reverse shutdown, loop control,
local and shared cleanup, optionals, arrays, calls, returns, and checked
success. The three sources compare default, `none`, each pass disabled, both
disabled, and post-proof unreachable deletion disabled. The failure source
pins panic and hard checked termination, including operand order, status,
exact source locations, and runtime-trace rows.

The checked-integer fixtures separately cover successful quotient, remainder,
and shift protocols for every primitive integer type, exact signed and width
boundaries, dynamic and effectful exclusions, nested expressions, static
lifecycle, and ownership destruction. Their failure matrix preserves operand
order and exact panic observations across default, `none`, checked-folding-
disabled, CFG-cleanup-disabled, and all-pass-disabled products.

Use `make golden-filter GOLDEN_FILTER='optimizations/**'` for the ordinary
focused suite. Use `scripts/golden.sh --determinism full --filter
'optimizations/**'` to repeat both compiler and native processes.
