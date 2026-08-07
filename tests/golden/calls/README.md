# Function-call fixtures

`functions.golden.toml` owns direct and nested calls, statement calls,
register and stack argument boundaries, returns, arity, and rejected call
targets. Register- and stack-pressure cases remain distinct sources because
they exercise different ABI boundaries.

Run this group with `scripts/golden.sh --filter 'calls/**'`.
