# Process fixtures

`arguments.golden.toml` owns exact process-argument observations. Its external
NUL-delimited manifest preserves empty, multiline, and non-UTF-8 arguments;
omitted stream expectations intentionally require exact empty output.

Run this group with `scripts/golden.sh --filter 'process/**'`.
