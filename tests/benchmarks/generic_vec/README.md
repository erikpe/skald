# Generic vector measurement workload

`growth.ska` exercises `Vec<i64>` zero-capacity growth through 4,096 elements,
independent structural copying, reverse popping, and clearing. Run the
reproducible measurement with:

```sh
make generic-vec-benchmark
```

The command reports compiler, assembly-emission, native-runtime, and artifact-
size observations. Host timing is deliberately not a correctness gate; the
workload itself exits unsuccessfully if its vector result is wrong.
