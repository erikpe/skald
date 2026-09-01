# Static-field fixtures

`fields.golden.toml` owns local and inherited static-field selection, mutation,
syntax, and type behavior. `modules.golden.toml` owns cross-module access and
the reachability-gated behavior for an imported but otherwise unused explicit
initializer and destructor; its provider sources remain undiscovered below
`cases/`.

Run this group with `scripts/golden.sh --filter 'static_fields/**'`.
