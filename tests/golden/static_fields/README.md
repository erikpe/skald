# Static-field fixtures

`fields.golden.toml` owns local and inherited static-field selection, mutation,
syntax, and type behavior. `modules.golden.toml` owns cross-module access; its
provider sources remain undiscovered below `cases/`.

Run this group with `scripts/golden.sh --filter 'static_fields/**'`.
