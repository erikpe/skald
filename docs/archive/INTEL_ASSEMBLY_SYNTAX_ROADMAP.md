# Intel-Syntax Assembly Roadmap

Status: complete.

Skald currently publishes deterministic GNU assembler text using AT&T
instruction syntax. The x86-64 backend should instead publish GNU assembler
text using Intel syntax with `noprefix`, matching the convention used by the
sibling Niflheim project and making emitted assembly easier to inspect for
contributors familiar with Intel notation.

This is an artifact-format migration at the final target-emission boundary.
It does not change target-independent MIR, instruction selection, executable
behavior, or the System V ABI.

## Scope and invariants

- Every `x86_64-sysv` assembly artifact begins with
  `.intel_syntax noprefix`; the backend does not offer an AT&T compatibility
  mode or a per-invocation syntax option.
- The existing private machine model continues to describe semantic sources,
  destinations, registers, memory operands, labels, and instruction kinds.
  Syntax-specific ordering and spelling remain owned by the text emitter.
- Intel instruction text uses destination-first operand order, bare register
  names, unprefixed immediates, bracketed memory operands, explicit memory
  widths where required, and RIP-relative forms such as
  `[rip + <symbol>]`.
- Conventional suffixless Intel mnemonics are used where operand widths make
  them unambiguous. Width-specific SIMD and general-register/XMM transfer
  mnemonics remain explicit where required by the GNU assembler.
- GNU/ELF directives and metadata—including `.globl`, `.type`, `.size`,
  `.quad`, alignment, relocation-data sections, and `.note.GNU-stack`—remain
  unchanged except for their position after the new syntax directive.
- Function and table symbols, label spelling, block order, section order,
  dispatch entries, frame offsets, and deterministic output remain unchanged
  unless their textual operand notation must change.
- Integer, byte, floating-point, direct-call, indirect-call, comparison,
  branch, stack, and RIP-relative instructions retain their current machine
  meaning. In particular, non-commutative operations must reverse textual
  operand order without reversing their semantics.
- Full-width `u64` immediates remain accepted. The existing decimal-versus-hex
  literal policy may be retained while the Intel `mov` form lets the GNU
  assembler select the required encoding.
- The driver continues streaming self-describing assembly to the configured C
  compiler driver with `-x assembler`; no host-tool syntax flag is introduced.
- The runtime archive and runtime ABI marker are unchanged. Hand-written
  assembly used only by backend tests adopts Intel syntax so combined test
  inputs do not depend on implicit dialect transitions.
- Existing source diagnostics, phase dumps, evaluation order, ownership,
  identities, target name, CLI paths, artifact publication guarantees, and
  native observations remain unchanged.
- Archived roadmaps retain their historical AT&T notation. Only living
  documentation is migrated.
- Adding another target, adding a general assembly-printer abstraction, or
  redesigning instruction selection is outside this roadmap.

## Progress

- [x] INTEL0 — Publish and verify Intel-syntax x86-64 assembly

## PR-sized implementation sequence

### INTEL0 — Publish and verify Intel-syntax x86-64 assembly

**Purpose:** Atomically replace the supported x86-64 textual assembly dialect
while preserving the target model, ABI, executable behavior, and deterministic
artifact boundary.

- [x] Make register renderers return bare Intel register names without `%`
      prefixes.
- [x] Start every emitted assembly program with
      `.intel_syntax noprefix`, followed by the existing text, function,
      dispatch-table, and non-executable-stack sections.
- [x] Render integer and SSE binary instructions in destination-first order,
      preserving the semantic `source` and `destination` fields in the private
      machine model.
- [x] Render immediates without `$`, indirect calls without `*`, frame and
      stack memory as bracketed operands, and symbol addresses as RIP-relative
      Intel operands.
- [x] Render memory widths explicitly where instruction operands do not
      provide a complete width, including byte stores and zero-extending byte
      loads; keep floating and general-register/XMM transfers assembler-valid.
- [x] Keep full-width immediate values, dispatch-table relocations, function
      metadata, local labels, and deterministic ordering accepted by the GNU
      assembler.
- [x] Update the exact minimal assembly expectation and all focused backend,
      MIR-boundary, and `skac` CLI assertions that encode AT&T instruction
      text.
- [x] Convert the hand-written output and floating-point validation stubs to
      Intel syntax, and keep the shared native-link helper in one explicit
      syntax mode for its complete combined input.
- [x] Add or retain focused assertions covering the syntax directive, integer
      and floating operand order, byte and qword memory widths, zero extension,
      RIP-relative addressing, full-width immediates, and indirect calls.
- [x] Update living backend, debugging, and repository overview documentation
      to call the artifact “GNU assembler text using Intel syntax with
      `noprefix`,” and use bare register notation when describing the current
      target.
- [x] Audit living code, tests, and documentation for residual AT&T register,
      immediate, memory, mnemonic, and indirect-call forms. Do not rewrite
      archived roadmaps.

**Tests:** Run
`cargo test --locked -p skald-compiler backend::x86_64_sysv::tests`,
`cargo test --locked -p skald-compiler mir::tests::type_operations`,
`cargo test --locked -p skald-compiler mir::tests::virtual_dispatch`,
and `cargo test --locked -p skac --test cli` while iterating. Confirm focused
assembler-acceptance and native-execution cases cover integer, byte, SSE,
stack-argument, dispatch, type-operation, and appended-stub paths. Before
completion, run `make check` and `git diff --check`. Run `make msrv-check` only
if the implementation introduces Rust syntax or manifest/toolchain changes
that could affect the declared minimum Rust version.

**Exit criteria:** `backend::emit_assembly` and `skac --emit asm` always produce
deterministic `.intel_syntax noprefix` text accepted by the system assembler;
all linked native observations and ABI boundaries remain unchanged; no living
expectation or documentation describes AT&T output; the full repository gate
passes; and no syntax-selection option, target-independent dependency, or
runtime ABI change has been introduced.

## Ordering and dependencies

INTEL0 has no dependency on another active roadmap. The syntax contract,
renderer, coupled test fixtures, and living documentation form one atomic
artifact migration: splitting them would leave either the implementation or
its checked-in contract knowingly stale. The work remains independent of
future shared-ownership and exception designs because it changes only the
final textual realization of the existing `x86_64-sysv` machine model.
