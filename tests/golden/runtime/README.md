# Runtime failure fixtures

These specs own explicit panic syntax and execution, allocation, array-bounds,
and string-bounds observations. Runtime failures require unsuccessful
termination and stable panic message prefixes while allowing richer future
stderr such as stack traces.

Run all runtime failures with `scripts/golden.sh --filter 'runtime/**'`, or
only explicit panics with `scripts/golden.sh --filter 'runtime/panic**'`.
