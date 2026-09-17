# Pre-migration compiler baseline

Complete paired foundation inputs for compiler commit
`9e3cebb172db1c5b0f7813ce7c34a87b019c9e9d`, captured on 2026-09-17.
The [measurement guide](../../../../docs/development/LOW_LEVEL_COMPILER_MEASUREMENTS.md#reviewed-pre-migration-baseline)
owns reviewed summaries, host limits, rebuild commands and qualification.

| File | Purpose |
| --- | --- |
| [`index.json`](index.json) | Revision and SHA-256 of all six evidence files |
| [`manifest.json`](manifest.json) | Exact frozen version-1 collection manifest |
| [`build-record.json`](build-record.json) | Clean source/profile/toolchain/flags attestation, binary/runtime hashes and exact commands |
| [`first.json.gz`](first.json.gz), [`second.json.gz`](second.json.gz) | Original full report bytes: every raw sample/event, provenance, identities, observations and per-callable metrics |
| [`observations.json.gz`](observations.json.gz) | Raw untimed trace stderr and native stdout/stderr for both roles/captures; UTF-8 strings preserve these workloads' bytes |
| [`comparison.json`](comparison.json) | Replayed per-workload gates; capture references point to the retained reports |

All identity/semantic/determinism and required metric evidence is complete.
The overall equivalent-build cost comparison remains **inconclusive** for
eleven short compile timings. No adoption cost exception or performance
improvement is claimed. Keep the recorded noise when collecting new evidence;
do not rewrite these historical samples. Assemblies/executables remain excluded
from Git; their hashes/static measurements are retained.

Verify record integrity and replay without running a benchmark:

```text
python3 scripts/verify_low_level_baseline.py tests/measurements/low_level_compiler/pre_migration_9e3cebb1
```

Run from the repository root. Zero means complete valid records, including the
retained inconclusive timing outcome. The comparison CLI's distinct exit rules
are documented in the guide. Retain this directory through foundation adoption;
later documentation archival must preserve its evidence and references.
