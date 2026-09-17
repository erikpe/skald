# Retained measurement inputs and evidence

[`local_mir_redundancy.toml`](local_mir_redundancy.toml) configures the opt-in
[final-MIR opportunity census](../../docs/development/MIR_REDUNDANCY_MEASUREMENT.md).

[`low_level_compiler/pre_migration_9e3cebb1/`](low_level_compiler/pre_migration_9e3cebb1/README.md)
retains the pre-migration foundation manifest, build attestation, full raw paired
reports, untimed diagnostics/native output and replayed comparison. Reports and
observations use ordinary gzip compression; no executable is checked in.
The [foundation measurement guide](../../docs/development/LOW_LEVEL_COMPILER_MEASUREMENTS.md)
owns the reviewed results, qualification limits and rebuild/comparison commands.

These are durable records, not benchmark runs in the ordinary test gate.
Their integrity and protocol replay are checked by `make measurement-support-test`.
