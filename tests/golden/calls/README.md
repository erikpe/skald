# Function-call fixtures

`functions.golden.toml` owns direct and nested calls, statement calls,
register and stack argument boundaries, returns, external signature types,
arity, and rejected call targets. Simple calls and aggregate result/receiver
pressure use distinct sources so their ABI obligations remain easy to inspect.

`aggregate_pressure.ska` combines a hidden object-result destination, receiver
identity, seven integer arguments, and nine floating arguments through direct
and interface calls. Weighted results detect argument loss or permutation;
later argument calls exercise preservation during evaluation. It runs with
default optimization, minimum optimization, and omitted runtime traces.

Run this group with `scripts/golden.sh --filter 'calls/**'`.
