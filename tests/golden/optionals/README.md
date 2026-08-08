# Optional-value fixtures

`values.golden.toml` owns primitive and alias presence, unwrap, conversion,
containment, syntax, and signature behavior. `lifecycle.golden.toml` owns
inline and shared payload lifecycle, direct-local optional shared-owner
transfer, produced optional shared-array result unwrapping and cleanup, and
guarded clear and replacement. Lifecycle output remains exact; native failures
use stable panic prefixes so future stack traces do not invalidate the owned
observation.

Run this group with `scripts/golden.sh --filter 'optionals/**'`. Use
`scripts/golden.sh --determinism full --filter 'optionals/**'` for a complete
guard and lifecycle audit.
