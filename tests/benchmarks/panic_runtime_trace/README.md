# Panic Runtime Trace Benchmark Fixtures

These successful, output-free programs isolate the runtime-trace execution
profiles owned by the
[performance procedure](../../../docs/development/PANIC_RUNTIME_TRACE_PERFORMANCE.md):

- `call_recursion.ska` repeatedly enters a short recursive source-call chain;
- `tight_loop.ska` performs no calls or panic-capable operations inside its
  hot loop; and
- `allocation.ska` repeatedly allocates and releases a shared object.

They are measurement inputs rather than pass/fail performance gates. Semantic
correctness remains covered by compiler and golden tests, and the benchmark
script rejects a build or run that exits unsuccessfully.
