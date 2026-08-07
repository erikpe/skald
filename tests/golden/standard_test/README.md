# Standard-test fixtures

`assertions.golden.toml` covers successful assertions and standard-test
failures. Failure cases match the owned diagnostic fragment without freezing
the surrounding panic rendering.

Run this group with `scripts/golden.sh --filter 'standard_test/**'`.
