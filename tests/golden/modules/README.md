# Module fixtures

`resolution.golden.toml` owns logical and positional entry selection, composed
provider roots, import cycles and bindings, visibility, malformed providers,
and provider collisions. Provider trees remain below `cases/` and are loaded
only through typed compiler arguments; their supporting `.ska` files are not
independent golden tests.

Focused diagnostics match the stable code, message, and relative primary
location. Multi-diagnostic and multi-provider observations retain exact
external stderr files. Run the group with `scripts/golden.sh --filter
'modules/**'` and audit diagnostic determinism with `scripts/golden.sh
--determinism compile --filter 'modules/**'`.
