# VM Benchmark Correctness Workload Roadmap

Status: planned; VB0 is next.

This roadmap ports the deterministic bytecode-VM regression workload from the
sibling Niflheim repository into a Skald-native multi-module golden test. The
durable outcome is one readable program that compiles a materially larger
source graph than the current focused fixtures, exercises interacting compiler
and runtime features in one invocation, and reports exact independent results
for twelve guest programs plus their aggregate.

Niflheim's guest instruction set, case algorithms, and expected observations
are the behavioral reference. Skald's implemented language, ownership model,
standard library, module system, and golden runner are authoritative. The port
must preserve the benchmark's validation value without recreating Niflheim's
implicit garbage-collected reference semantics.

## Scope and invariants

- Place one authoritative module graph below
  `tests/golden/vm_benchmark/cases/modules/` and invoke it through a logical
  module entry in `vm_benchmark.golden.toml`.
- Keep the seven purposeful VM responsibilities: entry/reporting, opcodes,
  model, instructions, builtins, runtime, and case construction.
- Preserve the twelve recognizable guest cases, their per-case observations,
  and the final aggregate: return value, memory value and checksum, trace sum,
  instruction count, call count, taken-branch count, builtin count, and mixed
  checksum.
- Preserve one compilation per selected variant with named runs for each case
  and the aggregate; do not split the source graph into independently compiled
  case fixtures.
- Model identity-bearing graph edges explicitly. Store instructions, builtins,
  erased constants, VM frames, and the program behind `shared` owners where
  identity or heterogeneous storage requires it; use `ref` and `mut ref` for
  non-owning call-scoped access.
- Keep benchmark cases and results as ordinary inline values. Keep function
  metadata inline where doing so preserves behavior without accidental
  identity or deep-copy dependence.
- Preserve VM frame semantics by sharing register backing and by saving frame
  owners, rather than copying mutable register state into independent arrays.
- Use explicit Skald initializers, `super(...)`, `virtual`/`override`, mutable
  methods, shared dereference, and owner-preserving casts. Do not emulate
  Niflheim references through unsafe external state.
- Replace Niflheim's nullable lazy static scratch object with an explicitly
  initialized non-null Skald owner. Remove `rt_gc_collect()` calls; the port
  must not add a tracing-GC compatibility API.
- Keep every guest computation deterministic and avoid time, randomness,
  unordered iteration, host-dependent output, and undefined overflow.
- Run the complete workload under `default`, `optimization-none`, and
  `omit-runtime-trace`. Do not duplicate `optimization-none` with the
  equivalent all-passes-disabled variant.
- Treat the workload as a correctness and regression stress program. Do not
  introduce timing thresholds or make host performance a pass/fail contract.
- Do not duplicate the module graph under `samples/`; document how to compile
  and run the golden-owned logical entry manually.
- Do not change language semantics, compiler architecture, runtime ABI, or the
  golden runner merely to ease the port. A genuine defect exposed by the
  workload must be isolated and fixed as an independently reviewed
  prerequisite rather than hidden inside a benchmark translation task.
- Do not turn this workload into exhaustive coverage for generics, optionals,
  maps, vectors, function values, failure diagnostics, or destruction order;
  focused owner tests remain authoritative for those contracts.

## Progress

- [ ] VB0 — Establish the Skald ownership model and minimal vertical slice
- [ ] VB1 — Port the instruction hierarchy and core VM workloads
- [ ] VB2 — Port heterogeneous constants, builtins, statics, and exact `f64`
- [ ] VB3 — Port the large algorithmic cases and aggregate verification
- [ ] VB4 — Add compiler variants, documentation, and full-suite hardening

## PR-sized implementation sequence

### VB0 — Establish the Skald ownership model and minimal vertical slice

**Purpose:** Settle the representation and module boundaries before hundreds
of construction sites depend on them, then prove those decisions through one
complete source-to-native case.

- [ ] Add `tests/golden/vm_benchmark/README.md`,
      `vm_benchmark.golden.toml`, and the seven-module provider tree below
      `cases/modules/vm_benchmark/`, plus a small logical entry module.
- [ ] Define `VmApi`, `Instruction`, and `Builtin` with exact read-only versus
      mutable method and `ref`/`mut ref` contracts; ensure instruction execution
      mutates the VM through a bounded interface view rather than copying it.
- [ ] Define the program, function, frame, case, result, and aggregate records
      with explicit initializers and documented inline/shared ownership at
      every graph edge.
- [ ] Represent heterogeneous instruction and builtin tables as arrays of
      shared interface owners and erased constants as shared `Obj` owners.
- [ ] Represent the active frame and saved-frame stack with shared frame
      owners, and give each frame shared register backing so call/return cannot
      accidentally deep-copy live registers.
- [ ] Port opcode identities, checksum mixing, the minimal instruction subset,
      the VM dispatch loop, and `slice1_minimal` without adding compatibility
      branches used only during rollout.
- [ ] Add the minimal named run with exact stdout matching the Niflheim
      observation after syntax-only naming adaptations.
- [ ] Document the ownership translation and a direct manual `skac --entry`
      invocation in the fixture README.

**Tests:** `make golden-filter GOLDEN_FILTER='vm_benchmark/**'`; rerun the
focused selection with `--jobs 1 --show-output`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** The logical module graph compiles with the installed standard
library, the minimal case produces its exact expected line, interface-dispatched
execution mutates one VM, call-scoped access does not copy it, frame/register
identity is explicit in source, and no compiler or runtime compatibility change
is included.

### VB1 — Port the instruction hierarchy and core VM workloads

**Purpose:** Establish the complete polymorphic execution surface and validate
the ordinary arithmetic, control-flow, guest-call, and memory paths before
adding erased host values and static state.

- [ ] Port the full base-instruction hierarchy with explicit Skald initializers,
      base initialization, virtual declarations, overrides, and shared
      interface storage.
- [ ] Preserve shared inherited execution bodies for register writes, binary
      and unary operations, comparisons, conditional branches, memory access,
      invocations, and returns so both inherited virtual and interface dispatch
      remain exercised.
- [ ] Port guest function metadata, call/return frame creation, argument
      transfer, return destinations, and stack-bound checks using the VB0 frame
      representation.
- [ ] Port `arithmetic_mixer`, `branch_maze`, `recursive_calls`, `dense_array`,
      and `slice_copy` without flattening their instruction streams or replacing
      their checks with host-side shortcuts.
- [ ] Preserve intrinsic array indexing, indexed writes, copied slices, slice
      replacement, integer casts, checked shifts, comparisons, loops, and
      branch counters used by these cases.
- [ ] Add one exact named golden run per newly ported case and retain the
      expected metadata beside each readable builder.

**Tests:** `make golden-filter GOLDEN_FILTER='vm_benchmark/**'`; focused runs for
each of the six available cases; `scripts/golden.sh --determinism full --jobs 1
--filter 'vm_benchmark/**'`; `make docs-check`; and `git diff --check`.

**Exit criteria:** All core instruction families compile through cross-module
inheritance and conformance, guest recursion restores the correct shared frame
and registers, array/slice cases retain their exact checksums, and the six
case-specific outputs are deterministic.

### VB2 — Port heterogeneous constants, builtins, statics, and exact `f64`

**Purpose:** Add the host-side type-erasure and standard-library surfaces only
after the VM's execution and ownership backbone is stable.

- [ ] Port `WeightSource`, `WeightTapeLike`, their implementations, and the
      builtin hierarchy with exact receiver mutability and structural indexing
      behavior.
- [ ] Translate constant pools to `(shared Obj)[]`, allocate the canonical
      Skald `BoxI64`, `BoxU64`, `BoxBool`, and `BoxF64` classes explicitly, and
      recover values with owner-preserving shared casts and pointee type tests.
- [ ] Replace the nullable lazy scratch interface and GC forcing calls with an
      eagerly initialized non-null shared static while preserving every numeric
      contribution to the expected outputs.
- [ ] Port string checksum inputs and rendering with `Str.concat`, canonical
      `Str.from_*` functions, and the explicit standard-output API; do not add
      string operators or convenience I/O solely for this fixture.
- [ ] Port `builtin_dispatch`, `obj_cast_builtin`, and `exact_double`, using
      binary-exact `f64` values and exact equality rather than approximate
      expectations.
- [ ] Add exact named runs for the three cases and verify that failed casts or
      dispatch mistakes cannot silently fall back to a default payload.

**Tests:** `make golden-filter GOLDEN_FILTER='vm_benchmark/**'`; focused runs for
the three builtin/type cases; existing focused shared-owner-array, shared
polymorphism, primitive-box, static-field, string-formatting, and structural
indexing goldens if shared fixture code changes expose a regression; `make
docs-check`; and `git diff --check`.

**Exit criteria:** Shared erased constants retain their concrete allocation and
cast correctly, builtin calls dispatch through shared interfaces, the static
scratch owner is valid from eager initialization through shutdown, all exact
`f64` assertions remain exact, and nine case outputs match their reviewed
expectations.

### VB3 — Port the large algorithmic cases and aggregate verification

**Purpose:** Add the largest instruction graphs and long-running guest control
flow after every instruction and host service they depend on is independently
covered.

- [ ] Port `prime_sum_100`, `fibonacci_recursive`, and
      `sha1_quick_brown_fox`, retaining readable builder helpers and comments
      for guest-loop structure, recursion, message schedule, and digest
      reconstruction.
- [ ] Preserve all patchable jump targets and ensure replacement of a
      placeholder instruction releases the superseded shared owner without
      changing the final instruction stream.
- [ ] Keep SHA-1 constants and shifts within Skald's explicit integer and
      checked-shift contracts and retain the independently reproducible digest
      explanation next to its golden expectation.
- [ ] Build the complete twelve-case inline case array and execute each case
      through the same checked VM path.
- [ ] Port aggregate accumulation and checksum mixing so every field of every
      result contributes exactly once in stable case order.
- [ ] Add the final three per-case runs and the aggregate run with exact stdout;
      do not replace the individual observations with only the aggregate.
- [ ] Review the completed source graph for opaque generated-looking regions
      and add narrow comments or helpers where intent is not recoverable from
      the instruction sequence.

**Tests:** `make golden-filter GOLDEN_FILTER='vm_benchmark/**'`; run all twelve
case selections and the aggregate with `--show-output`; independently compare
the SHA-1 digest and every aggregate field with the Niflheim reference;
`scripts/golden.sh --determinism full --jobs 1 --filter 'vm_benchmark/**'`;
`make docs-check`; and `git diff --check`.

**Exit criteria:** All twelve cases and the aggregate produce reviewed exact
output, the aggregate equals the stable ordered fold of the individual results,
the largest instruction graph stays understandable, and repeated compilation
and execution are byte-deterministic.

### VB4 — Add compiler variants, documentation, and full-suite hardening

**Purpose:** Make the completed workload a durable repository gate across the
compiler configurations it is intended to protect, and prove that its size
does not destabilize ordinary development workflows.

- [ ] Select `default`, `optimization-none`, and `omit-runtime-trace` in the
      golden spec and reuse the identical thirteen named runs and exact
      observations for every variant.
- [ ] Confirm optimized and unoptimized MIR preserve every per-case and
      aggregate value; keep pass-specific selection mechanics in their focused
      optimization fixtures.
- [ ] Measure and record compile, link, run, assembly-size, and slowest-leaf
      observations for the three variants as non-normative maintenance data in
      the fixture README.
- [ ] Confirm every native run remains within the golden runner timeout and
      that parallel execution introduces no artifact or resource collision.
- [ ] Update `docs/development/TESTING.md` to identify the VM benchmark as the
      broad multi-module correctness workload and document focused,
      deterministic, optimized, and unoptimized commands.
- [ ] Audit the fixture against current grammar, ownership, module, standard
      library, driver, and golden-runner documentation; update living docs only
      where the port reveals a genuine missing description of current behavior.
- [ ] Run the complete repository quality gate and inspect failures for hidden
      shared-owner leaks, stale frame retention, nondeterministic output, or
      assumptions masked by one optimization profile.
- [ ] Remove roadmap task codes and rollout wording from fixture source,
      comments, names, and living documentation before closure.

**Tests:** `make golden-filter GOLDEN_FILTER='vm_benchmark/**'`; the same filter
with `--jobs 1`, full determinism, and each selected variant; `make check`;
`make golden-release-test`; `make golden-determinism-test`; `make msrv-check`
only if Rust sources, manifests, or supported syntax changed while resolving a
genuine prerequisite; `make docs-check`; and `git diff --check`.

**Exit criteria:** Every one of the 39 variant/run leaves passes with exact
output, full determinism and release-built tools pass, ordinary repository
validation remains reliable, the fixture README records non-normative cost and
manual-use guidance, living testing documentation identifies the workload,
and no benchmark-only compiler or runtime accommodation remains.

## Ordering and dependencies

VB0 fixes the only representation decision that would otherwise churn every
later constructor, field, call, and array. VB1 completes the ordinary VM before
VB2 introduces erased values and static ownership, keeping dispatch failures
separable from cast and standard-library failures. VB3 then adds the largest
case builders without inventing new execution machinery. VB4 multiplies the
workload across compiler variants only after one complete default-profile
program is stable, avoiding repeated expensive builds during structural work.

The roadmap depends only on implemented Skald language, standard-library,
module, driver, and golden-runner contracts. It has no dependency on the
optimization candidates currently cataloged under `docs/roadmaps/`; the
finished benchmark instead becomes useful validation for later optimization
roadmaps. If an implementation defect blocks a task, isolate its reproducer,
record the dependency, and resolve it independently before resuming the port.
