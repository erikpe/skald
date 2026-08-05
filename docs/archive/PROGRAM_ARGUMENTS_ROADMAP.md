# Program Arguments Roadmap

Status: complete.

This roadmap gives native Skald applications an explicitly imported
`std::process::args() -> Str[]` API while preserving the established
parameterless entry function. The implementation baseline has advanced since
the initial investigation: raw-byte strings now provide checked byte access
and shared-backing slices, `std::io::read_file` is implemented through verified
byte-array I/O, primitive text conversion is ordinary standard-library code,
zero-default static fields are available, and runtime ABI version 8 is current.
Those additions make a source-only implementation over Linux
`/proc/self/cmdline` preferable to the previously considered entry-wrapper and
runtime-accessor extension.

The sibling Niflheim implementation is useful evidence for returning the
invocation name at index zero, preserving empty arguments, and decoding the
NUL-separated procfs record in standard-library code. Skald retains its own
module, ownership, error, test, and documentation boundaries rather than
copying Niflheim's `std.io` placement or vector-based splitting machinery.

## Scope and invariants

- Add exactly this initial public source surface in logical module
  `std::process`:

  ```ska
  public fn args() -> std::str::Str[];
  ```

- `std::process` remains outside any prelude. Source must import the module or
  function through the ordinary module system before calling it.
- Preserve `fn main() -> i64` as the only valid entry signature. Arguments are
  obtained by an ordinary library call; type checking, HIR, MIR, target entry
  selection, and the internal callable ABI do not gain a special entry
  parameter.
- Return the complete process invocation vector in host order. Element zero is
  the host-supplied invocation name, not a promised canonical, absolute, or
  existing executable path. A host invocation with no vector entries produces
  an empty array.
- Preserve every argument as an exact finite sequence of non-NUL bytes,
  including empty arguments, spaces, tabs, newlines, and non-UTF-8 bytes.
  `Str` remains a raw-byte value; the API performs no shell tokenization,
  quoting, escaping, Unicode decoding, locale conversion, or normalization.
- The current Linux implementation reads `/proc/self/cmdline` through the
  implemented `std::io::read_file` function. The procfs record is interpreted
  as NUL-terminated arguments; consecutive terminators produce empty
  arguments, and the terminal delimiter does not create an additional value.
- Treat a nonempty procfs record without its required final NUL as a host
  contract violation rather than inventing a portable alternate encoding or
  silently widening the public API into a generic string-splitting function.
- Read and parse the procfs record once per `args()` call. Each call returns a
  fresh owning `Str[]`; no static field, lazy cache, top-level initialization,
  or mutable process-global library state is introduced.
- Count arguments before allocating the result array, then scan the same
  captured `Str` again and assign one checked `Str.slice` per record. Slices may
  share the captured string's backing internally while ordinary synthesized
  ownership keeps that backing alive after the local capture is destroyed.
  Backing identity and exact allocation count remain unobservable.
- Use existing `Str.len`, `Str.byte`, `Str.slice`, inline-array construction,
  exact-class element assignment, `while`, and primitive arithmetic/casts.
  Do not add a vector, iterator, `for` loop, builder, split method, byte-view
  escape, string-layout privilege, or process-specific compiler intrinsic.
- Inherit the existing all-or-panic `std::io::read_file` failure behavior when
  procfs is absent or cannot be opened, read, or closed. This profile adds no
  process-specific panic message or recoverable error value.
- Keep runtime ABI version 8, `ska_rt_abi_v8`, the five byte-I/O functions, the
  generated C-compatible `main` wrapper, and the public C header unchanged.
  No C `argc`/`argv` capture, retained raw pointer, new runtime global, or ABI
  marker transition is part of this roadmap.
- Keep grammar, resolution, type checking, HIR, MIR, verification, backend
  legality, layout, symbols, and assembly emission unchanged. The new module is
  ordinary reachable standard-library source over already implemented
  language features.
- Do not use zero-default static fields to cache the vector. Their process
  lifetime and final-cleanup exclusions are unnecessary for a small explicit
  snapshot API and would make repeated-call behavior stateful.
- Extend the native golden harness with an optional exact-byte `.argv`
  sidecar. It contains only arguments after element zero, encoded as a sequence
  of NUL-terminated byte strings. Absence or an empty file means no additional
  arguments; one NUL encodes one empty argument; a nonempty file without a
  final NUL is invalid fixture data.
- Keep `.argv` distinct from `case.args`: the former configures the generated
  executable, while the latter continues to configure the `skac` invocation
  for a multi-file case. A multi-file native case uses `case.argv`.
- Preserve exact non-UTF-8 fixture bytes by constructing Unix `OsString`
  values directly. This is consistent with the repository's sole Linux target
  and must not introduce lossy UTF-8 conversion in the runner.
- Supply the same working directory, `.argv`, and `.stdin` inputs to both
  deterministic native executions before comparing status, stdout, and
  stderr.
- Environment variables, current-directory accessors, executable-path
  canonicalization, argument mutation, process spawning, exit APIs, signal
  handling, Windows command-line reconstruction, non-Linux argument discovery,
  general collections, and general string splitting remain non-goals.
- The repository Makefile remains the validation interface. This roadmap adds
  no CI configuration.

## Progress

- [x] PA0 — Freeze the process-argument source and host contract
- [x] PA1 — Add exact executable arguments to native goldens
- [x] PA2 — Implement and harden `std::process::args`

## PR-sized implementation sequence

### PA0 — Freeze the process-argument source and host contract

**Purpose:** Settle the public module path, invocation-vector meaning, raw-byte
behavior, Linux discovery mechanism, ownership, failure policy, and unchanged
compiler/runtime boundary before test or library representations depend on
them.

- [x] Add `docs/language/PROCESS.md` as the source authority for
      `std::process::args()`, explicit import, element-zero meaning, byte
      preservation, empty arguments, repeated snapshots, ownership, procfs
      decoding, inherited I/O failures, costs, and the exclusions above.
- [x] Record clearly that the current implementation requires the existing
      Linux `/proc/self/cmdline` record and that a later target may replace the
      library implementation only while preserving the public vector and byte
      semantics.
- [x] Update the documentation authority index, language overview, status
      matrix, modules/entry-point contract, standard I/O cross-reference, and
      standard-library guide. Mark the feature as a frozen design rather than
      implemented until PA2 completes.
- [x] State in the appropriate living authorities that the feature composes
      existing declarations, calls, arrays, loops, strings, modules, and I/O;
      it changes neither the implemented grammar nor any compiler phase.
- [x] Reconcile runtime/backend prose only where needed to make the unchanged
      ABI version 8 and unchanged parameterless process wrapper explicit. Do
      not add a speculative C argument API.

**Tests:** Run `make docs-check`; review non-archived matches from
`rg -n "std::process|process arguments|/proc/self/cmdline|\.argv" docs std tests -g '*.md' -g '*.ska' -g '*.rs'`;
and run `git diff --check`.

**Exit criteria:** One linked language authority answers every public semantic,
ownership, failure, and host-discovery question needed by implementation;
living documentation accurately identifies the feature as frozen and leaves
`fn main() -> i64`, runtime ABI version 8, and all compiler phases unchanged.

### PA1 — Add exact executable arguments to native goldens

**Purpose:** Establish a deterministic, byte-exact observation path for
command-line arguments before the standard-library API relies on it.

- [x] Extend the native golden input model and sidecar loader with optional
      `.argv` bytes, parsing NUL-terminated records into `OsString` values by
      using the Linux/Unix byte-preserving conversion API.
- [x] Reject a nonempty `.argv` file without a final NUL with a focused fixture
      error. Preserve consecutive delimiters, leading/trailing empty arguments,
      spaces, line feeds, and non-UTF-8 bytes exactly.
- [x] Pass only the decoded additional arguments to `Command`; leave the
      operating system and Rust process API responsible for element zero.
      Apply the same arguments to both repeated executions and retain the
      existing working-directory, piped-stdin, output-capture, and deadlock
      protections.
- [x] Keep native input parsing with the existing native-expectation owner and
      process orchestration in the golden runner. Extract a new module only if
      the resulting files mix independently understandable parsing and
      execution responsibilities; do not create a one-function namespace.
- [x] Add focused Rust tests for missing and empty sidecars, one empty
      argument, multiple arguments with whitespace, consecutive empty
      arguments, exact non-UTF-8 bytes, malformed missing termination, and
      independence from `case.args`.
- [x] Update the golden fixture guide and testing authority with the `.argv`
      format, single-file and `case.argv` naming, exact-byte guarantees, and
      the distinction between compiler and executable arguments.

**Tests:** Run `cargo fmt --all`, `make golden-expectations-test`,
`make golden-run-test`, `make static-check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** Any native golden can supply an exact Linux argument vector
after element zero, including empty and non-UTF-8 values; malformed fixtures
fail deterministically; both native executions receive identical inputs; and
all existing cases behave unchanged when `.argv` is absent.

### PA2 — Implement and harden `std::process::args`

**Purpose:** Publish the ordinary Skald standard-library implementation over
the now-testable Linux process record and reconcile all living documentation
with executable behavior.

- [x] Add `std/std/process.ska` with ordinary imports of `std::io` and
      `std::str` and the exact public function frozen in PA0. Add no intrinsic,
      external declaration, static state, or private compiler convention.
- [x] Read `/proc/self/cmdline` through a named `Str` local eligible for the
      existing `ref` parameter, count NUL terminators with checked byte access,
      allocate the exact `Str[]` length, and fill it in source order with
      checked shared-backing slices.
- [x] Preserve an empty procfs record as an empty vector, leading and
      consecutive NULs as empty arguments, and the final terminator as the end
      of the last argument rather than a new argument. Keep all scan indices
      within the established `Str`/array `i64` position boundary.
- [x] Add the canonical module source to only those compiler test fixtures
      that promise the complete installed standard-library closure or directly
      exercise `std::process`. Do not expand this task into the separately
      tracked canonical-fixture centralization cleanup.
- [x] Add focused provider/pipeline coverage proving that the installed module
      is reached through ordinary imports and compiles through verified MIR to
      assembly without a new runtime symbol, entry-wrapper call, or ABI marker.
- [x] Add native goldens for no additional arguments and for a vector
      containing ordinary text, whitespace, empty, and non-UTF-8 arguments.
      Avoid asserting the build-directory spelling of element zero; assert its
      vector position/count and the exact supplied suffix.
- [x] Promote process arguments to implemented status and update
      `docs/language/PROCESS.md`, the status and language indexes,
      modules/entry-point prose, standard I/O cross-reference, standard-library
      guide, and testing guide to describe only current behavior.
- [x] Audit generated assembly and the public runtime header to confirm that
      runtime ABI version 8 and the parameterless internal Skald entry call are
      byte-for-byte unchanged. Remove any stale frozen-rollout wording outside
      the roadmap.

**Tests:** Run focused compiler provider/pipeline tests,
`make golden-expectations-test`, `make golden-run-test`, `make docs-check`,
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** An installed-standard-library program can import and call
`std::process::args()` and observe the complete raw-byte Linux invocation
vector with correct array/string ownership; exact native suffix arguments pass
twice deterministically; all living documentation says implemented; no
compiler phase, process entry signature, runtime symbol, or ABI version has
changed; and the full repository gate passes.

## Ordering and dependencies

PA0 comes first because module placement, element-zero meaning, byte behavior,
procfs dependence, and the decision not to revise the runtime ABI determine
every later representation. PA1 then gives the repository a reusable exact
native-input boundary independent of the library implementation. PA2 can use
that boundary to land the small source module and complete behavior hardening
without mixing test-runner design into the public API change.

The roadmap depends on the already implemented raw-byte string, inline-array,
whole-file standard I/O, module-system, and Linux x86-64 profiles. It has no
dependency on another active roadmap. The pending canonical standard-library
fixture and standard-I/O test-organization discoveries remain separate:
implementation may add the new module to existing complete fixture lists, but
must not absorb either cleanup into this feature.
