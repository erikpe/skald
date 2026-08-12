# Optional-value fixtures

`values.golden.toml` owns compositional nesting, optional arrays, checked
array-payload aliases, primitive and alias presence, unwrap, conversion,
containment, syntax, and signature behavior. `lifecycle.golden.toml` owns
inline and shared payload lifecycle, canonical and shorthand optional-owner
equivalence, direct-local optional shared-owner transfer, produced optional
shared-array result unwrapping and cleanup, and guarded clear and replacement.
`boxes.golden.toml` owns the complete shared optional-box source matrix,
including primitive and aggregate targets, deep optional owners, object views,
interface dispatch, stored and array positions, owner replacement, compile-time
exclusions, absent unwrap, and failed checked casts.
Lifecycle output remains exact; native failures use stable panic prefixes so
future stack traces do not invalidate the owned observation.

Run this group with `scripts/golden.sh --filter 'optionals/**'`. Use
`scripts/golden.sh --determinism full --filter 'optionals/**'` for a complete
guard and lifecycle audit.
