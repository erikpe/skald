# Standard I/O Maintainability Discoveries

Status: pending.

This record keeps a test-organization improvement discovered during the IO7
closeout audit outside the completed standard I/O roadmap. It does not affect
the language, compiler, runtime ABI, or test behavior.

## Extract native I/O probe assembly from the behavior suite

- **Problem:** `backend/x86_64_sysv/tests/io.rs` mixes the behavioral assertions
  with roughly half a module of assembly-source probe builders. At more than
  680 lines, this makes the supported behaviors and failure cases harder to
  scan even though the production backend responsibilities remain concise.
- **Evidence:** `read_failure_probe`, `binary_read_probe`, `unused_read_probe`,
  `invalid_write_probe`, `partial_binary_write_probe`, and
  `closed_descriptor_write_probe` own self-contained assembly fixtures rather
  than test decisions.
- **Boundary:** Move only those private fixture builders into a sibling
  `io_probes.rs` test module with narrow `pub(super)` constructors. Keep the
  behavior tests and assertions in `io.rs`; do not change generated assembly,
  runtime behavior, public APIs, or compiler module boundaries.
- **Priority:** Low. The current suite is deterministic and cohesive by
  feature; this is a navigation improvement for a future focused cleanup.
