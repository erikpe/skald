# Foundation measurement corpus

`manifest.json` freezes workload version 1 for low-level architecture comparisons.
It reuses the four compile families and range/vector/trace kernels from the
cleanup harness, and adds `scalar_calls.ska` for the scalar/call pilot.
The kernel takes runtime arguments `17 1000000` and prints `-993156\n` under
both trace policies. Expected observations are independent of emitted assembly.

The [measurement procedure](../../../docs/development/LOW_LEVEL_COMPILER_MEASUREMENTS.md)
defines paired collection, provenance, metrics, regression decisions and durable
retention. Change workload inputs deliberately and requalify evidence; do not
silently substitute incompatible runs into an existing comparison.
