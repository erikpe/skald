# Standard I/O Roadmap

Status: in progress; IO0 and IO1 are complete, and IO2 is next.

This roadmap establishes a small handle-and-byte runtime boundary and uses it
to implement whole-input and whole-output functions in Skald's standard
library. The durable result is a `std::io` module whose public API deals in
`Str`, while compiler-known private intrinsics deal only in aliased `u8[]`
storage and the C runtime sees only scalar handles plus byte pointers and
lengths.

## Scope and invariants

- The public source surface is exactly:

  ```ska
  public fn read_stdin() -> Str;
  public fn read_file(ref path: Str) -> Str;
  public fn write_stdout(ref text: Str) -> unit;
  public fn write_stderr(ref text: Str) -> unit;
  ```

- These declarations live in logical module `std::io`, remain outside any
  prelude, and require an ordinary explicit import or qualification.
- `read_stdin` reads until EOF without closing stdin. `read_file` opens its
  path read-only, reads until EOF, and closes the opened handle after a
  successful read before returning. Empty input produces an empty `Str`.
- Reads preserve every byte exactly, including zero and non-UTF-8 bytes.
  `Str` remains the language's finite raw-byte string rather than acquiring
  text decoding, normalization, or host-locale semantics.
- `write_stdout` and `write_stderr` submit exactly the bytes in `text`, in
  order, without adding a line ending, buffering in C streams, or closing the
  standard handle. Empty output succeeds without a host write.
- The current public functions are all-or-panic operations. Host open, read,
  write, and close failures use the exact catalog messages
  `io: failed to open file`, `io: failed to read file`,
  `io: failed to read stdin`, `io: failed to write stdout`,
  `io: failed to write stderr`, and `io: failed to close file` as applicable.
  Buffer-growth overflow uses `io: input too large`, while an impossible
  oversized result or zero-progress nonempty write uses
  `io: invalid runtime result`. No host error number or path is appended.
- A panic remains non-returning and non-unwinding. An opened handle is closed
  on the normal successful file-read path; no new cleanup guarantee is made
  after an I/O failure begins termination.
- The private canonical `std::io` intrinsic declarations are:

  ```ska
  intrinsic fn _io_standard_handle(stream: u8) -> i64;
  intrinsic fn _io_open(ref path: u8[], mode: u8) -> i64;
  intrinsic fn _io_read(handle: i64, mut ref destination: u8[], offset: u64) -> i64;
  intrinsic fn _io_write(handle: i64, ref source: u8[], offset: u64) -> i64;
  intrinsic fn _io_close(handle: i64) -> i64;
  ```

- Intrinsics are selected by exact resolved identity and exact signature, not
  by a leaf name. They are private implementation details of `std::io` and do
  not widen the ordinary external-function ABI or allow arbitrary source to
  declare compiler intrinsics.
- Standard-stream selectors are `0u8` for stdin, `1u8` for stdout, and `2u8`
  for stderr. Open mode `0u8` means read an existing path. Other selectors and
  modes are rejected by the runtime boundary; no public mode constants or
  handle values are exposed in this profile.
- `_io_open` receives the complete path byte array. On the current Linux
  target, paths are raw host pathname bytes; an embedded zero is invalid.
- `_io_read` and `_io_write` evaluate their arguments once from left to right.
  They require `offset <= array.len()`, anchor the selected array backing for
  the operation, and expose only the remaining range beginning at `offset`.
  Read access is mutable and write/open access is read-only. A bounds failure
  uses the existing array-bounds panic contract before entering C.
- Every intrinsic returns `i64`. Non-negative values mean success. Open and
  standard-handle success return a handle; read and write success return the
  number of bytes transferred; read result zero means EOF; close success
  returns zero. Negative values mean a host failure. Exact negative codes are
  compiler/runtime-private and are not portable language or public standard-
  library semantics.
- Runtime ABI version 7 adds `ska_rt_io_standard_handle`, `ska_rt_io_open`,
  `ska_rt_io_read`, `ska_rt_io_write`, and `ska_rt_io_close`. Those functions
  receive no `Str`, array descriptor, shared owner, allocator policy, or
  source-level I/O object. Open alone adapts a length-delimited byte path to
  the host's terminated pathname convention.
- The exact added C surface is:

  ```c
  int64_t ska_rt_io_standard_handle(uint8_t stream);
  int64_t ska_rt_io_open(const uint8_t* path, uint64_t path_length, uint8_t mode);
  int64_t ska_rt_io_read(int64_t handle, uint8_t* destination, uint64_t capacity);
  int64_t ska_rt_io_write(int64_t handle, const uint8_t* source, uint64_t length);
  int64_t ska_rt_io_close(int64_t handle);
  ```

- On the current POSIX runtime, an ordinary host failure is returned as the
  negative `errno` value; embedded-zero and unrepresentable paths use an
  appropriate negative host-style error. Standard-library code observes only
  the sign. Invalid standard-stream selectors, unsupported open modes, and
  invalid pointer/length pairs are private compiler/runtime contract defects,
  not ordinary source I/O failures.
- Runtime read and write perform at most one successful host transfer, return
  partial progress, retry interruption before any progress, and cap an
  oversized request to the host transfer limit. Skald code owns EOF loops,
  buffer growth, partial-write loops, contextual failure selection, and
  conversion between arrays and `Str`.
- Runtime close is attempted once and reports its host result; it is not
  blindly retried after an indeterminate interrupted close. The standard
  library never closes standard handles.
- The existing `Str.to_bytes()` and `Str.from_bytes(ref bytes: u8[])` copying
  operations are used unchanged. This roadmap deliberately accepts their
  intermediate copies rather than adding byte views, owned-byte adoption,
  builders, or another string representation capability.
- Primitive-to-string and string-to-primitive formatting or parsing are
  excluded. The existing bootstrap observability helpers
  `ska_rt_println_i64`, `ska_rt_println_u64`, `ska_rt_println_u8`,
  `ska_rt_println_f64_bits`, and `ska_rt_println_bool`, their tests, samples,
  and semantics remain present and unchanged.
- Fine-grained public streams, files, handles, seeking, file writing, append,
  creation, metadata, directories, line-oriented I/O, flushing, buffering,
  asynchronous/nonblocking I/O, recoverable `IoError`/`Result` values,
  exceptions, and platform-independent path objects are non-goals.
- No new source grammar is required. Existing module, function, alias, array,
  loop, intrinsic-declaration, and panic syntax composes the feature.

## Progress

- [x] IO0 — Freeze standard I/O language and compiler contracts
- [x] IO1 — Publish the byte-oriented runtime ABI
- [ ] IO2 — Resolve and type private standard I/O intrinsics
- [ ] IO3 — Represent and verify target-independent I/O operations
- [ ] IO4 — Lower verified I/O to the x86-64 runtime ABI
- [ ] IO5 — Implement exact standard-stream writes
- [ ] IO6 — Implement whole stdin and file reads
- [ ] IO7 — Harden integration and reconcile living documentation

## PR-sized implementation sequence

### IO0 — Freeze standard I/O language and compiler contracts

**Purpose:** Establish the source behavior, intrinsic identities, byte and
handle boundary, failure policy, ABI transition, and explicit exclusions
before implementation phases depend on them.

- [x] Add `docs/language/IO.md` as the authority for the four public functions,
      raw-byte behavior, EOF and exact-output semantics, path behavior,
      normal handle ownership, panic catalog, costs, and exclusions above.
- [x] Add `docs/compiler/IO.md` as the authority for canonical intrinsic
      validation, HIR/MIR ownership, array access and backing anchors, offset
      checks, verification, x86-64 pointer/length realization, private result
      encoding, and the version-7 C ABI direction.
- [x] Update the documentation, language, and compiler indexes, the status
      matrix, standard-library guide, phases/IR overview, backend guide,
      runtime ABI, errors, arrays, strings, modules/interoperation, and testing
      guide so they link the frozen design while accurately retaining current
      compiler/runtime behavior as not yet implemented.
- [x] State explicitly in the implemented grammar that this feature composes
      existing syntax and adds no token, precedence, or declaration form.
- [x] Preserve formatting/parsing, new `Str` conversions, owned-byte adoption,
      replacement of observability output, and broader I/O as separate future
      work rather than allowing them to enter implementation tasks.

**Tests:** Run `make docs-check` and review non-archived matches from
`rg -n "std::io|read_stdin|read_file|write_stdout|write_stderr|io_(open|read|write|close)" docs std -g '*.md' -g '*.ska'`.

**Exit criteria:** One source contract and one compiler/runtime contract are
linked from living indexes, all representation-level decisions above are
settled, current versus frozen status is unambiguous, documentation validation
passes, and no executable behavior has changed.

### IO1 — Publish the byte-oriented runtime ABI

**Purpose:** Land and independently verify the small host-I/O boundary before
compiler-generated calls depend on it, without removing or changing bootstrap
observability.

- [x] Advance the public runtime ABI, version-specific marker, generated entry
      reference, inspection hook, direct contract tests, backend expectations,
      and stale-runtime link-mismatch tests from version 6 to version 7.
- [x] Add scalar/pointer C declarations for
      `ska_rt_io_standard_handle`, `ska_rt_io_open`, `ska_rt_io_read`,
      `ska_rt_io_write`, and `ska_rt_io_close` without exposing a Skald array
      descriptor or string layout.
- [x] Implement standard-handle selection and read-only open using raw POSIX
      descriptors. Copy and terminate a checked length-delimited pathname,
      reject embedded zero and unrepresentable length, request close-on-exec,
      and leave pathname allocation as host-adaptation rather than a retained
      runtime object.
- [x] Implement one-transfer read/write wrappers with zero-length behavior,
      partial progress, `EINTR` retry, `SSIZE_MAX` request capping, null/length
      contract checks, negative `errno` host failures, EOF, and no `FILE *` buffering
      or flushing.
- [x] Implement one-attempt close and keep selector, mode, handle, pointer, and
      length contract defects separate from ordinary negative host failures.
- [x] Add focused direct C harnesses using pipes and temporary files for exact
      standard handles, empty/binary reads and writes, EOF, partial progress,
      invalid paths/modes/descriptors, normal close, and post-close failure.
- [x] Keep all five `ska_rt_println_*` implementations, declarations, exact
      record tests, failure tests, and documentation intact under ABI version
      7.
- [x] Update the runtime Makefile, runtime test guide, and ABI authority with
      the new harness ownership and exact current surface.

**Tests:** `make runtime-test`, focused driver link-mismatch tests, backend
marker tests, `make docs-check`, and `git diff --check`.

**Exit criteria:** Version 7 independently exposes five tested byte-oriented
I/O operations, version 6 fails the link guard, existing allocation, panic,
and observability behavior is byte-for-byte unchanged, and canonical compiler
intrinsic lowering does not call the new services yet. Ordinary trusted
primitive-only `extern fn` assertions remain governed by the existing foreign
interoperation contract.

### IO2 — Resolve and type private standard I/O intrinsics

**Purpose:** Give each operation a stable canonical semantic identity and
checked source call shape without widening foreign interoperation or relying
on lower-phase name lookup.

- [ ] Refactor the singleton panic-intrinsic validator into a cohesive closed
      intrinsic registry that preserves panic behavior and recognizes only the
      five exact private declarations in logical module `std::io` with the
      signatures above.
- [ ] Extend intrinsic linkage identities with the five I/O operations and
      preserve deterministic module, function, parameter, and intrinsic
      identities through resolved IR and declaration dumps.
- [ ] Add a canonical `std/std/io.ska` module containing its imports and the
      five private bodyless intrinsic declarations; public function bodies
      remain for the later standard-library tasks.
- [ ] Diagnose wrong module paths, visibility, names, parameter names, modes,
      array element types, alias access, arity, result types, ordinary
      definitions, external declarations, and any unrecognized intrinsic
      declaration without weakening the exact panic diagnostic.
- [ ] Type intrinsic calls into dedicated I/O HIR rather than leaving them as
      ordinary direct calls. Reuse existing array-alias capability checking so
      open/write require read access, read requires mutable access, produced
      arrays and copied slices remain ineligible aliases, and scalar arguments
      retain exact types and left-to-right evaluation.
- [ ] Permit every `i64`-returning intrinsic only in ordinary expression
      consumers and keep the declarations definition-free and symbol-free in
      target-independent declaration metadata.
- [ ] Add focused resolver/type-check tests and deterministic resolved/HIR
      dumps for valid calls, invalid declarations, invalid alias sources,
      inaccessible private imports, replacement standard libraries, and an
      unrelated source module attempting to manufacture an intrinsic.

**Tests:** Focused syntax/resolution/type-check intrinsic and array-alias
tests, pipeline determinism for valid and invalid `std::io` providers,
`make compiler-test`, and `make docs-check`.

**Exit criteria:** Exactly five private canonical I/O identities select typed
I/O HIR with correct scalar and array access, malformed or noncanonical
declarations fail before HIR, panic remains unchanged, and no target-dependent
symbol or array layout has entered HIR.

### IO3 — Represent and verify target-independent I/O operations

**Purpose:** Make evaluation, backing lifetime, bounds safety, access, result
carriage, and runtime-call intent explicit and independently verifiable before
target lowering.

- [ ] Add a cohesive MIR I/O instruction family for standard-handle, open,
      read, write, and close operations with explicit scalar inputs, exact
      `u8[]` places, offsets, access modes, results, and spans.
- [ ] Lower HIR arguments exactly once from left to right, retain the array
      backing through the complete operation using existing array-alias anchor
      machinery, and end temporary/alias lifetimes in normal full-expression
      order.
- [ ] Lower open against the complete read-only array range. Lower read/write
      offsets through the ordinary checked array-position boundary, permit an
      offset equal to length for an empty remaining range, and take the
      existing array-bounds failure edge before any runtime call when the
      offset is larger.
- [ ] Represent the intrinsic result as one initialized exact `i64` value and
      preserve the private negative-error convention without embedding POSIX
      `errno`, target pointers, descriptor offsets, or symbol names in MIR.
- [ ] Extend MIR dumps and verification for exact operation/input/result
      types, compatible read-only versus mutable array access, live initialized
      descriptors and backing anchors, dominated bounds checks, one result
      definition, and absence of residual ordinary calls to intrinsic
      declarations.
- [ ] Add malformed-MIR mutations for wrong operation types, non-`u8` arrays,
      access escalation, dead/detached backing, missing anchors/checks,
      out-of-order lifetimes, duplicate or absent results, and calls through
      intrinsic declaration metadata.
- [ ] Require every backend to support the verified I/O family or reject it
      structurally; do not let a target infer operation kind from source names.

**Tests:** Focused HIR-to-MIR lowering, exact MIR dumps, array alias/anchor and
storage-lifetime tests, malformed-verifier mutations, backend structural
rejection tests, and `make compiler-test`.

**Exit criteria:** Verified target-independent MIR proves safe synchronous
access to one remaining `u8[]` range and one exact scalar result for every I/O
operation, while corrupted access, bounds, lifetime, and intrinsic-call forms
are rejected before instruction selection.

### IO4 — Lower verified I/O to the x86-64 runtime ABI

**Purpose:** Realize the verified pointer/length boundary mechanically on the
current target and connect it to the independently tested version-7 runtime.

- [ ] Add x86-64 System V instruction selection for the five MIR operations,
      using the verified array descriptor and offset to compute a backing byte
      address and remaining length without passing the descriptor or owner.
- [ ] Marshal selectors, modes, handles, pointers, lengths, and signed `i64`
      results under the existing target call machinery and preserve live
      array-backing anchors across each C call.
- [ ] Reference only the exact `ska_rt_io_*` symbol selected by the MIR
      operation and retain name-independent target-independent IR.
- [ ] Preserve zero-length ranges without dereferencing their data pointer,
      canonical scalar result storage, stack alignment, caller-saved state,
      and existing full-expression cleanup around the host call.
- [ ] Add backend legality and assembly tests for each operation, offset zero,
      dynamic offsets, empty remaining ranges, bounds failure before C,
      register pressure, alias backing anchors, and exact runtime symbols.
- [ ] Add native compiler tests with small private standard-library fixtures
      that exercise successful scalar results and host failures without yet
      claiming the four public `std::io` functions.
- [ ] Update backend, phases/IR, compiler I/O, debugging, and runtime ABI
      documentation from frozen representation direction to implemented
      compiler/runtime intrinsic support while keeping the public standard
      library status planned.

**Tests:** Focused x86-64 I/O selection and native tests, system-assembler
acceptance, driver linkage with the version-7 archive, `make runtime-test`,
`make compiler-test`, and `make docs-check`.

**Exit criteria:** Every verified I/O operation emits a valid version-7 call
with the correct byte range and signed result, malformed/unimplemented forms
remain rejected, and the public standard-library functions are the only
remaining feature work.

### IO5 — Implement exact standard-stream writes

**Purpose:** Publish the two output functions as ordinary Skald loops over the
generic byte-array write intrinsic while retaining the bootstrap print API.

- [ ] Implement `write_stdout(ref text: Str) -> unit` and
      `write_stderr(ref text: Str) -> unit` in `std/std/io.ska`, using the
      existing `Str.to_bytes()` copy and one private `_write_all` helper over a
      selected standard handle.
- [ ] Skip the intrinsic for an empty array; otherwise loop over partial
      results by advancing an offset until all bytes are written.
- [ ] Panic with the exact stream-specific failure on a negative result and
      with `io: invalid runtime result` on zero progress or a count larger than
      the remaining range. Do not append LF, flush, close, or retry in Skald
      after the runtime has reported failure.
- [ ] Preserve array anchoring and cleanup across repeated calls and ensure
      output completes before the temporary byte array is destroyed.
- [ ] Add native goldens for empty, ordinary, embedded-zero, embedded-newline,
      non-UTF-8, repeated, and capacity-crossing output to each standard
      stream, plus focused failure tests with closed descriptors.
- [ ] Keep the five observability helpers and all existing sources, runtime
      harnesses, native goldens, stdout records, and failure behavior intact;
      do not migrate samples or compiler tests to `std::io` in this task.
- [ ] Update the standard-library guide and focused I/O/status/testing
      documentation to mark only the two write functions implemented.

**Tests:** Focused type/HIR/MIR/backend tests for the canonical module,
write-success and closed-descriptor native tests, `make golden-test`,
`make runtime-test`, `make compiler-test`, and `make docs-check`.

**Exit criteria:** Imported public writes emit exactly the supplied `Str`
bytes through partial-write-safe Skald loops, all write failures terminate
through the selected standard-library panic, and observability output remains
fully supported and unchanged.

### IO6 — Implement whole stdin and file reads

**Purpose:** Complete the requested public API with one Skald-owned growable
read-all algorithm and deterministic byte-exact stdin and file evidence.

- [ ] Implement one private read-all helper in `std/std/io.ska` using an
      initial fixed-capacity `u8[]`, a filled offset, repeated reads until
      result zero, checked geometric growth, and existing array copy/slice
      operations.
- [ ] Validate every result before updating the filled range; use the selected
      read failure on a negative result, `io: invalid runtime result` when a
      count exceeds remaining capacity, and `io: input too large` when growth
      cannot produce a larger valid array.
- [ ] Implement `read_stdin() -> Str` from the standard stdin handle without
      closing it. Implement `read_file(ref path: Str) -> Str` by copying the
      path through existing `to_bytes`, opening read-only, reading all bytes,
      closing normally, and only then returning a `Str` made through existing
      `from_bytes`.
- [ ] Keep the deliberate intermediate and final byte copies visible in tests
      and documentation; do not add an owned-array string factory, byte view,
      builder, or compiler-visible `Str` layout shortcut.
- [ ] Extend native golden expectations with an optional byte-exact `.stdin`
      sidecar, feed the same bytes to both deterministic executions, cover
      missing-as-empty behavior and binary mismatch reporting, and avoid pipe
      deadlock for larger inputs.
- [ ] Add stdin goldens for empty, embedded-zero/non-UTF-8, and input crossing
      the initial growth boundary. Add working-directory file fixtures for
      empty and binary files, growth, raw-byte preservation, and normal close.
- [ ] Add focused failure coverage for nonexistent and embedded-zero paths,
      unreadable/open failure, read failure, close failure where it can be
      induced deterministically, and input-growth overflow at its owning
      compiler/standard-library boundary.
- [ ] Update golden, standard-library, language I/O, status, and testing
      documentation to describe `.stdin`, whole-read blocking/EOF behavior,
      file handle ownership, and the complete four-function public API.

**Tests:** `make golden-expectations-test`, focused stdin/file native goldens,
canonical standard-library type/HIR/MIR tests, `make golden-test`,
`make runtime-test`, `make compiler-test`, and `make docs-check`.

**Exit criteria:** Empty and arbitrary binary stdin/files round-trip exactly to
`Str`, growth and EOF are correct, successful files close before return,
deterministic native tests can supply stdin, and no new string conversion or
public fine-grained I/O API has appeared.

### IO7 — Harden integration and reconcile living documentation

**Purpose:** Close cross-layer gaps, preserve explicit exclusions, and prove
the complete feature from canonical source through the versioned runtime
without folding later formatting or observability migration into this plan.

- [ ] Add complete deterministic resolved/HIR/MIR/assembly coverage for a
      program importing all four functions, including replacement and disabled
      standard-library providers and exact private-intrinsic visibility.
- [ ] Audit intrinsic validation so panic plus the five I/O operations form
      one closed canonical registry with no leaf-name recognition, forged
      declarations, residual intrinsic calls, or foreign array ABI escape.
- [ ] Audit large functions and modules added by the feature by responsibility;
      keep intrinsic selection, I/O HIR/MIR, verification, target lowering,
      runtime host adaptation, and standard-library algorithms behind concise
      facades.
- [ ] Audit source, runtime, samples, tests, and living documentation to ensure
      no claim says primitive formatting moved to `Str`, observability helpers
      were removed, `Str` gained zero-copy conversion, or public streaming,
      recoverable errors, file writing, or path abstraction is implemented.
- [ ] Reconcile language/compiler I/O, strings, arrays, errors, modules,
      phases/IR, backend, runtime ABI, standard-library, status, testing,
      debugging, README, and documentation-index wording with current
      implemented behavior and one authoritative owner per fact.
- [ ] Confirm public symbols, the ABI-v7 marker, runtime archive contents,
      standard-library module paths, exact golden bytes, and absence of build
      artifacts or unrelated diffs.
- [ ] Run the complete repository and supported-toolchain gates from an
      artifact-free snapshot, complete every roadmap checkbox, and archive the
      roadmap with repaired index links.

**Tests:** `make check`, `make msrv-check`, focused cross-process pipeline
determinism, runtime symbol inspection, `git diff --check`, documentation-link
validation, and repository-status inspection.

**Exit criteria:** The four canonical public functions execute through
verified byte-array intrinsics and the version-7 handle runtime, all living
contracts and tests agree, existing observability remains unchanged, every
exclusion above remains absent, and the roadmap is ready to archive.

## Ordering and dependencies

IO0 settles source, intrinsic, failure, and ABI contracts before code depends
on them. IO1 publishes the runtime boundary independently. IO2 then assigns
canonical semantic identities and checked HIR; IO3 establishes verified
target-independent execution and array lifetime safety before IO4 realizes
the target ABI. IO5 lands output first so IO6 can use it to prove byte-exact
read round trips while extending the golden runner for stdin. IO7 closes
cross-layer gaps and audits the deliberately deferred work.

The roadmap depends only on the completed module, panic, string, array-alias,
loop, primitive-operator, and runtime-versioning foundations. It has no
dependency on another active roadmap. Runtime and contract work may be
prepared independently, but compiler lowering must not call the new C ABI
until IO1 is complete, and public standard-library bodies must not land until
verified backend support is complete.
