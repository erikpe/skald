# Tight range-loop measurement workloads

These six successful, output-free programs compare immediately fused `u8`,
`u64`, and `i64` ranges with handwritten `while` loops containing the same
current/end/item storage, advance-before-body order, checksum work, and
iteration counts. Each executable validates its own deterministic checksum.

Run the reproducible measurement with:

```sh
make range-loop-benchmark
```

The procedure, inspection guidance, environment details, and recorded
reference result are maintained in
[Tight Range-Loop Performance](../../../docs/development/RANGE_LOOP_PERFORMANCE.md).
Generated artifacts are ignored under `build/measurements/range-loop/`.
