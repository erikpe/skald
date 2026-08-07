# Private-initializer module fixtures

`construction.golden.toml` owns class-local access to private initializers.
`modules.golden.toml` owns rejected foreign construction through an imported
class. The diagnostic matches its stable identity and relative primary
location while allowing richer renderer context.

Run this group with `scripts/golden.sh --filter
'private_initializers/modules**'`.
