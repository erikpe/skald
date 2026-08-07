# Scripts

Repository workflow scripts belong here when repeated build, golden-test, or
release tasks justify them. Core compiler behavior must remain available
through `skac`, and validation must remain available through the Makefile
rather than existing only in shell scripts. See the
[development workflow](../docs/development/README.md).

`golden.sh` builds `skac` and `skald-golden`, changes to the repository root,
and forwards every argument unchanged to the Rust golden runner. It is useful
for inspection and combinations of filters that do not need a dedicated Make
target:

```text
scripts/golden.sh --list --filter 'run/**'
scripts/golden.sh --exact 'run/strings::default::<run>' --show-output
scripts/golden.sh --filter 'compile_fail/**' --determinism compile
```

The Makefile remains authoritative for complete ordinary and full-determinism
validation through `make golden-test` and `make golden-determinism-test`.
