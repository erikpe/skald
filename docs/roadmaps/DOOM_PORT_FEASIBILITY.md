# Doom in Skald: feasibility assessment

Status: actionable investigation; no port or language-extension roadmap started.
Next step: settle a minimal native shared-array bridge and numerical helpers, then
build a window/input/framebuffer spike before porting the engine.

## Assessment and scope

**A playable, silent, single-player proof of concept is feasible.** Skald has
most of the necessary computational features today. This is nevertheless a
substantial engine rewrite, not the small platform adaptation normally meant
by a Doomgeneric port. Doomgeneric makes the operating-system boundary small;
it does not remove the engine's dependence on C pointers, layouts, and arithmetic.

Recommended initial target: Linux x86-64, one explicitly selected classic Doom
IWAD, a 320×200 software framebuffer, keyboard movement/turning/strafing/use,
wall collision, stairs, doors, lifts, switches, keys, and level changes.
Choose a small set of stock maps and explicitly enumerate their required
specials. Arbitrary WAD compatibility is a separate goal. An empty-world camera
viewer is a useful intermediate result but does not establish normal navigation.

Remove audio output, music, sound resources/configuration, networking, network
checksums, multiplayer state, and synchronization. Also defer save/load, demos,
configuration persistence, menus, automap, intermission/finale presentation,
DeHackEd, WAD merging, and broad game-version compatibility. These additional
cuts are recommendations for the proof of concept, not requirements implied
by removing audio/networking. Monsters, weapons, and combat can follow the
navigation milestone. Maps with combat-dependent progression need a documented
shortcut or must stay outside the initial acceptance set.

Keep WAD parsing, texture/patch decoding, palette lookup, BSP traversal,
rasterization, collision, simulation, and map transitions in Skald. The native
library should own only window/display/event/timer services and transfer of
already-rendered pixels. Do not retain `r_*`, `p_*`, or WAD-decoding C code behind
a platform API. Generic host services in Skald's existing runtime are compatible
with this boundary.

## Evidence and scale

Inspected local Doomgeneric revision
`dcb7a8dbc7a16ce3dda29382ac9aae9d77d21284` and Skald revision
`e044178afb2ddbeb36c2a230c62e3a7f794a1f5c` on 2026-09-11. Doom paths below are
relative to `../doomgeneric/doomgeneric/`; that sibling is external to this
repository's documentation links.

Physical source line counts include comments, tables, and disabled branches;
they are scope indicators, not translation estimates:

| Source group | C files | Lines |
| --- | ---: | ---: |
| Default Makefile's source list | 81 | 55,913 |
| All top-level C files, including alternative platforms | 95 | 62,463 |
| `r_*.c` and `v_video.c` | 9 | 6,506 |
| `p_*.c` and `g_game.c`, including deferred gameplay/save code | 20 | 17,815 |
| `w_*.c` | 5 | 1,088 |
| `tables.c` and `info.c` | 2 | 6,889 |

The renderer and world/game groups alone contain about 24,000 lines before
pruning. Dropping audio/networking will not remove the hardest dependencies.
Large constant tables can be mechanically converted to Skald source; that
reduces transcription work, not the need to validate state/action mappings.
Expect a multi-stage project, plausibly months for one developer, rather than
a weekend platform port. This is a judgment about scope, not a measured schedule.

Principal source evidence:

- `doomgeneric.h`, `doomgeneric.c`, `i_video.c`, `i_input.c`, `i_timer.c`:
  native hooks, framebuffer ownership, palette conversion, input, timing.
- `doomfeatures.h`, `d_main.c`, `d_loop.c`, `d_net.c`: feature switches,
  initialization, command generation, and simulation scheduling.
- `doomdata.h`, `p_setup.c`, `w_wad.c`, `w_file_stdc.c`: packed disk records,
  little-endian conversion, map graph construction, lump caching.
- `r_defs.h`, `r_data.c`, `r_draw.c`, `r_bsp.c`: pointer-rich render state,
  patch composition, column/span loops, and BSP visibility.
- `m_fixed.c`, `m_fixed.h`, `tables.h`: 16.16 fixed point and 32-bit binary angles.
- `d_think.h`, `p_tick.c`, `info.h`, `info.c`: intrusive lists, erased callbacks,
  delayed removal, actor state tables.
- `p_map.c`, `p_maputl.c`, `p_spec.c`, `p_doors.c`, `p_plats.c`, `p_enemy.c`:
  collision/traversal, specials, and the distinction between audio and AI alerts.

Skald evidence includes the [status matrix](../language/STATUS.md),
[grammar](../language/GRAMMAR.md), [backend contract](../compiler/BACKEND.md),
[external-signature checker](../../crates/skald-compiler/src/typeck/program/mod.rs),
[array ABI checks](../../crates/skald-compiler/src/typeck/arrays/validation.rs),
[standard I/O implementation](../../std/std/io.ska), and existing array-alias
and function-value golden cases. The sibling Niflheim Traci feasibility record
was also consulted for staged-port planning; Skald remains authoritative.

## Platform boundary

The original hooks are not directly expressible as-is:

| Doomgeneric surface | Skald adaptation |
| --- | --- |
| `DG_Init`, `DG_DrawFrame` | Primitive-only wrappers are straightforward, but drawing needs pixel transfer |
| `DG_SleepMs(uint32_t)`, `DG_GetTicksMs()` | Wrap with exact `u64`/`uint64_t` signatures; do not declare a C `uint32_t` result as Skald `u64` |
| `DG_GetKey(int*, unsigned char*)` | Return one packed event integer with explicit empty/pressed/released/quit encoding |
| `DG_SetWindowTitle(const char*)` | Start with a fixed title, or use the proposed borrowed bytes |
| Global `DG_ScreenBuffer` pointer | No source-visible foreign global/raw-pointer access; transfer to native-owned display storage |

Skald should own `main` and poll the host. No C-to-Skald callback or public
native export is necessary. Handle held keys, releases, window close, and focus
loss explicitly. The native adapter can implement the DG hooks internally;
there is no benefit in forcing their exact C signatures into the Skald surface.
C is sufficient; C++ would need an `extern "C"` wrapper boundary.

The original engine renders indexed pixels at 320×200; the default DG display
buffer is 640×400 and usually uses 32-bit pixels. Keep the port's low-resolution
buffer and let display scaling happen at presentation. For the strict engine
boundary above, perform palette-to-RGBA conversion in Skald and submit explicit
RGBA bytes. Host packing to a window library's required pixel format is display
adaptation. Do not make the shim interpret WAD palettes or draw Doom columns.

**Possible without new language features:** pack eight bytes into each `u64`
and pass offset plus word through primitive externs. A 320×200 indexed frame
requires 8,000 calls; an RGBA frame requires 32,000 calls. This is a functional
fallback, not evidence of acceptable whole-game performance. Per-pixel calls
are also possible but add needless boundary overhead.

**Reusable longer-term addition:** a narrow, call-scoped borrowed byte-array FFI contract,
e.g. a future external parameter lowered to `const uint8_t*` plus `uint64_t`
length. This is proposed behavior, not currently accepted syntax. Define
argument expansion, empty buffers, lifetime/backing anchoring, and forbidden
retention/reentry/mutation before implementing it. Start read-only for display;
add mutable buffers only with a separately checked write contract. A copy into
native presentation storage before return is sufficient. Existing private
`std::io` array intrinsics demonstrate related machinery, but are not an
extension mechanism for application-defined externs.

### Smaller proof-of-concept option: expose the shared-array handle

A version-pinned native adapter that decodes a shared byte array is reasonable
for this proof of concept. A general borrowed-buffer FFI need not come first.
The [current shared-array layout](../compiler/ARRAYS.md#initial-x86-64-shared-outer-layout)
is one contiguous allocation on x86-64:

| Byte offset from handle | Contents |
| ---: | --- |
| 0 | Eight-byte strong count |
| 8 | Eight-byte metadata/finalizer pointer |
| 16 | Eight-byte element count |
| 24 | First byte of a `shared u8[]` payload |

The **handle itself is already a pointer** to this allocation. Passing the
handle avoids a second indirection. Passing a pointer to its owner slot would
require C to load the handle first; these must not be confused. This layout is
specific to shared byte arrays, not inline arrays or arbitrary shared objects.
RGBA can remain a byte array, so no `u32` support is needed for presentation.

A native function receiving the handle could read the length with `memcpy`
from `(const unsigned char *)handle + 16` and consume bytes at `handle + 24`.
It should verify the expected frame length and synchronously copy/upload the
frame, leaving ownership and metadata alone. Keep a Skald owner live through
the call and do not retain the address after return. All layout knowledge can
live in one adapter function. Keep it paired with the compiler revision;
the public runtime ABI marker alone does not promise this private layout.

**The remaining obstacle is exporting the pointer, not decoding it.** Current
Skald rejects external shared-array and alias parameters, and provides no
shared-owner-to-integer/raw-address cast. A probe declaring
`extern fn present(ref pixels: shared u8[]) -> unit` produces `TYP009` for both
the array ABI and alias ABI exclusions. Declaring a primitive `u64` parameter
does not make a shared owner a legal argument.

The smallest credible compiler change is a deliberately narrow, recognized
presentation intrinsic/bridge accepting the array and passing a borrowed handle
to one C symbol while preserving its live owner for the call. Alternatively,
design a limited external shared-array borrow. Neither should silently use an
ordinary by-value owning handoff that would leave C responsible for Skald
release. Review signature checking, effects, lifetime and native argument
lowering together; disabling the frontend rejection alone is insufficient.
This is proposed work, not an existing user-extensible intrinsic facility.

For this project's limited ABI requirement, prefer this small bridge over
blocking the port on a general FFI design. No compiler implementation or native
handle-transfer test is included in this assessment.

The CLI currently offers assembly emission and a fixed runtime link input,
not arbitrary native library flags. A project build can emit assembly and link
it with the shim and runtime using `cc`. Dedicated link-input support would
improve ergonomics but is not a blocker. See
[driver/toolchain settings](../compiler/DRIVER_AND_ARTIFACTS.md#host-toolchain-and-runtime-selection).

## Numerical semantics: the most important correctness trap

Doom's `fixed_t` is signed 32-bit 16.16; `angle_t` is unsigned 32-bit with one
turn equal to 2^32. Skald currently provides `i64`, `u64`, and `u8`, with
wrapping arithmetic and explicit casts. Simply replacing every C `int` with
`i64` changes behavior. Angles must wrap at 32 bits; signed intermediate
narrowing and overflow-sensitive tests need deliberate treatment.

There is also a division mismatch: Skald signed `/` floors, while the C integer
division used by `FixedDiv` truncates toward zero. For example, `-7 / 3` is
`-3` in Skald and `-2` in C. Skald remainder follows the divisor's sign.
Adding `i32` alone would not fix that difference.

A practical current-language baseline:

- Keep fixed-point values in sign-normalized `i64`; normalize a low signed
  32-bit result with `(value << 32u) >> 32u` where required.
- Keep angles in `u64`, masking with `0xffffffffu` after relevant arithmetic
  and before ordering/table-index operations.
- Use wide multiplication followed by arithmetic shift for `FixedMul`;
  preserve the saturation guard and truncating division for `FixedDiv`.
- Implement truncating division with unsigned magnitudes and one unsigned
  division, restoring the sign afterward; see the measured code-generation
  probe below. Preserve Skald's existing signed division semantics.
- Decode signed 16-bit coordinates and signed/unsigned 32-bit disk fields
  explicitly from bytes. Preserve sentinel meanings such as missing back sides
  and tagged BSP children rather than blindly making every index unsigned.

Prefer this fixed-point baseline to changing the whole engine to `f64` during
translation. Floating point is possible, but would combine a numerical redesign
with a language port and still leave angle masks, table indexes, and bit flags.
Visual inaccuracy is tolerable; broken clipping and collision are not.

### Helper cost and division frequency

No change to Skald `/` or `%` semantics is proposed. This ordinary helper uses
unsigned magnitude arithmetic, including a representable magnitude for the
minimum signed integer:

```ska
fn trunc_div(a: i64, b: i64) -> i64 {
    var ua: u64 = (u64) a;
    var ub: u64 = (u64) b;
    if (a < 0) { ua = 0u - ua; }
    if (b < 0) { ub = 0u - ub; }
    var q: u64 = ua / ub;
    if ((a < 0) != (b < 0)) { return (i64) (0u - q); }
    return (i64) q;
}
```

Zero uses the existing division-by-zero failure. The minimum-signed/-1 pair
wraps through the explicit cast, rather than reproducing C undefined behavior;
it is outside Doom's guarded fixed-division domain. `FixedDiv` must still
implement its original saturation guard before calling this helper.

Compiled with the current compiler, default optimizations and
`--omit-runtime-trace`, the helper contains **one `div rcx` and no `idiv`**.
There are sign branches, casts/moves, a zero check and an ordinary call frame;
the emitted function also has substantial stack traffic. It is not equivalent
to one bare machine instruction and no cycle/frame-rate claim has been measured.
A linked C oracle checked all 40,200 nonzero-divisor pairs over [-100,100], plus
one negative 48-bit numerator representative of scaled fixed-point input:
40,201 comparisons, exit 0. The probe lives in `/tmp/skald-doom-feasibility/`.

The earlier quotient-plus-remainder formulation is correct for the relevant
operands but can issue two divisions without quotient/remainder fusion. The
unsigned-magnitude helper avoids relying on that optimization. It can also be
written directly inside the port's `FixedDiv` wrapper to avoid a nested helper
call, with the same mathematical contract.

Preprocessing the local C sources with the default Makefile's feature defines
and excluding the header declaration gives these active `FixedDiv` call sites:

| Files | Sites |
| --- | ---: |
| `r_main.c` | 7 |
| `r_plane.c` | 2 |
| `r_things.c` | 2 |
| `p_map.c` | 12 |
| `p_maputl.c` | 5 |
| `p_setup.c` | 1 |
| `p_sight.c` | 3 |
| **Renderer and world total** | **32** |

These are static call-site counts, not operations per frame. Preprocessing
matters: raw searches also find unused `#if 0` versions and floating debug code.
Some renderer sites initialize tables; others execute per visible segment,
sprite, frame or traversal. Most `p_map.c` fixed divisions serve aiming/shooting
and can disappear from a navigation-only port.

Direct signed `/` expressions also need review: wall scale interpolation in
`r_segs.c`, negative movement halving in `p_mobj.c`, aim-slope averaging in
`p_map.c`, and projectile vertical slopes in `p_mobj.c`/`p_enemy.c`. Positive
sizes/counts and many compile-time constants need no truncation workaround.
The wall-column reciprocal divisions in `r_segs.c` are explicitly unsigned.
The core pixel loops in `r_draw.c` use additions, shifts, masks and table loads,
not signed division per pixel. This makes a helper a reasonable initial choice;
real frequency and performance require profiling actual maps.

### Scope of native 32-bit integer support

`i32`/`u32` would be a cross-compiler feature, larger than the private framebuffer
bridge. Existing `u8` width handling provides a useful model, but primitive
families are explicitly enumerated at multiple layers:

- Source type/literal spelling, ranges, casts, diagnostics, and resolved types.
- HIR/MIR numeric kinds, capability rules, operators, signed division semantics,
  shift bounds, verifiers, and textual dumps.
- Constant evaluation/folding, algebraic identities, and cast simplification,
  preserving 32-bit wrapping and sign/zero extension at every step.
- Native loads/stores, four-byte field/array layouts, arithmetic and comparison,
  parameter/results across ordinary and foreign calls, optionals and lifecycle.
- Generic composition, standard-library conversion/boxing policy, documentation
  and source-to-native regression coverage. A complete primitive cast matrix
  grows from 25 pairs to 49 if both types are added to the current five.

A responsible implementation is several focused PRs, plausibly weeks of work
including validation rather than a small syntax patch; no implementation-based
schedule has been established. Signed `/` should continue to floor, including
for `i32`. Use fixed-point helpers first unless broader language use makes the
32-bit feature worth doing now. No 16-bit primitives are required to decode WAD
coordinates, which can be sign-extended from bytes.

## Representation and engine changes

**WADs:** existing `std::io::read_file` returns a binary-safe `Str`, including
embedded NULs. An entire IWAD can be read and indexed today. Parse headers,
directories, fixed-width names, map records, texture directories and patch posts
in Skald; do not reinterpret byte memory as C structs. Validate offsets/counts
against the buffer and record sizes. Use offset/length descriptors over one
backing store, preserve lump lookup precedence when overlays are later added,
and bound patch-column decoding. Packed structs, pointer casts, `sizeof`, and
native endianness support are unnecessary. Streaming/seek and a direct byte-file
API could reduce startup copies but are not prerequisites for a stock IWAD.

**World graph:** replace vertex/line/side/sector/subsector pointers with indices
into level-owned arrays. This matches WAD references naturally and avoids
strong ownership cycles. Own all per-level state together and release it on
transition; replace `Z_Malloc` tags/purge semantics with explicit lifetime and
cache ownership. Do not create a `shared` reference for every graph edge.

**Mutable entities:** use stable slots/IDs, explicit inactive state, and deferred
removal. Avoid moving live entities while IDs refer to them; use generations if
slots are reused with surviving references. Preserve thinker ordering and
spawn/removal behavior. These semantics matter even when only doors and lifts
remain. Separate pools per thinker kind or integer action dispatch avoid C's
union of incompatible function pointers and sentinel function addresses.

**Arrays and aliases:** Skald already supports class arrays and call-scoped
`ref`/`mut ref` array and class-element parameters. Pass large world/render
objects by alias. Ordinary owning array copies and slices copy storage;
`Vec<T>` getters return values rather than mutable element views. Do not
translate a C interior pointer into repeated slicing or copy-modify-forget
access through a vector. Use built-in arrays and helper calls for hot mutable
state. Borrowed local spans would be convenient, but offset/length plus alias
parameters are sufficient.

**Callbacks and tables:** capture-free function values exist, including class
fields, but raw arrays/optionals of functions and foreign callbacks are excluded.
Doom's heterogeneous `actionf_t` is unsuitable for direct translation anyway.
Integer action IDs with explicit dispatch are a robust first representation.
A class wrapping a typed callback is attractive in principle, but the array
composition bug reproduced below must be fixed before relying on it.

**Control flow:** `while`, `for`, `break`, and `continue` are implemented.
C `switch`, fallthrough, `goto`, `do/while`, increment and compound assignment
need restructuring. Enums/constants can initially use named static fields and
integer IDs. These are translation costs, not expressive blockers.

**No networking:** `FEATURE_MULTIPLAYER` is already undefined, yet `d_net.c` and
`d_loop.c` still participate in local startup and ticking. Replace their retained
responsibilities with a one-player command stream and a 35 Hz accumulator;
render independently and bound catch-up after stalls. Removing the files without
replacing these responsibilities does not produce a working single-player loop.

**No sound:** remove audio calls, data, drivers and configuration, not merely
mute a backend. `P_NoiseAlert`/sector `soundtarget` in `p_enemy.c` is AI awareness,
not audio playback. Under a literal removal of everything sound-related, omit
it too and use sight-only awareness if monsters are added. Retaining renamed
alert propagation would be a deliberate gameplay decision, not required for the
navigation proof of concept.

## What to add before starting

| Priority | Change | Recommendation |
| --- | --- | --- |
| First | Minimal borrowed shared-byte-array handle bridge | Recommended proof-of-concept boundary: one native adapter decodes the private layout; general byte-buffer FFI can follow later |
| First | Numerical helper module and focused conformance tests | Required engineering work even without language changes: signed normalization, wrapping angles, truncating division, fixed math, endian reads |
| Optional before porting | `i32` and `u32` primitives with casts/operators/array/ABI coverage | Reduces arithmetic-porting risk, but requires several compiler-wide changes; tested helpers allow the port to start first |
| Complete | Function-valued fields in class-array lifecycle emission | The x86-64 copy helper and callback-identity regression are complete; typed callback tables are available |
| Useful next | Named enum values and exhaustive dispatch/match | Helps state IDs, events, entity kinds and specials; not essential for a reduced port |
| Optional | Public binary byte I/O and build link inputs | Existing whole-file I/O and assembly linking suffice |
| Defer | Local borrowed spans, compound assignment, richer constants | Useful convenience/performance tools; justify after real port experience |

Do not require raw pointers, unions, packed layouts, a garbage collector, weak
references, closures, threads, C callbacks, exceptions, or a new backend for this
proof of concept. The current supported backend limits the initial deployment
target despite Doomgeneric's own platform portability.

## Executed probes and discovered defect

Built the compiler from the current checkout with `cargo run --quiet --locked
-p skac -- ... --emit asm`, linked generated assembly with a small C observer
and `build/runtime/libskald_runtime.a`, and ran it successfully (exit 0).
The temporary probe in `/tmp/skald-doom-feasibility/` checks:

- reading a synthetic 12-byte IWAD header containing NUL bytes;
- signed 32-bit normalization, unsigned angle wrap and negative truncating division;
- a function-valued field on one ordinary class value;
- filling a 64,000-byte array through `mut ref`;
- transferring all bytes in 8,000 packed `u64` calls, checked byte-for-byte by C.

This establishes those narrow paths, not real WAD parsing, graphical output,
large-program compilation, or Doom frame rate. No actual level was run.

The archived [compiler crash report](../archive/FUNCTION_FIELD_ARRAY_COMPILER_CRASH.md)
records the reproduction, cause, repair, and regression coverage. Before that
repair, a minimal accepted-source example crashed native array lifecycle
emission, even when only constructing and reading the array:

```ska
fn action(value: i64) -> i64 { return value; }
class State {
    callback: fn(i64) -> i64;
    init() { self.callback = action; }
}
fn main() -> i64 {
    var states: State[] = State[](2u);
    return states[1].callback(0);
}
```

The defective revision exited with status 101 and
`verified scalar copy has a primitive payload` in
[array lifecycle emission](../../crates/skald-compiler/src/backend/x86_64_sysv/lower/array/lifecycle.rs).
The repair adds function values to its full-word scalar path. The regression
proves callback identity across class-array copying, assignment, slicing, and
independent mutation.

## Suggested delivery gates

These are acceptance milestones, not PR-sized implementation tasks. Split each
into a focused roadmap only after choosing its representations and scope.

1. **Host and arithmetic spike:** display an animated 320×200 Skald-generated
   framebuffer, consume key press/release/quit events, and measure frame transfer
   and allocation behavior. Differentially test fixed math and angle boundaries
   against the source where its C operations are well-defined.
2. **Asset and map loading:** read the selected IWAD, resolve palette/colormap,
   textures/flats/patches and one map; compare decoded counts/references to the C
   version. Reject truncated/out-of-range records. A 2D map display helps validate
   geometry before perspective rendering.
3. **Stationary textured view:** BSP traversal, wall clipping, floors/ceilings,
   sky and masked surfaces at a known player start. Compare several reference
   camera views; exact pixels are not required. This is the main renderer gate.
4. **Navigable world:** fixed-rate player input, collision/sliding, stairs,
   height constraints, doors, lifts, use/cross-line switches and required keys.
   Validate motion under slow rendering and stalled-window recovery.
5. **Multiple levels:** exits or an explicit level-selection control, complete
   per-level teardown/reload, and repeated navigation through the chosen maps.
   Add sprites and any required remaining specials; defer full combat unless
   separately selected.

A useful provisional playability gate is sustained roughly 20–35 displayed
frames/second at native render resolution, responsive input, and 35 simulation
tics/second on a named development machine. This is an acceptance proposal,
not a performance prediction. Measure loading/compile time, memory and frame
cost as the renderer grows; current small-program success cannot establish
whole-engine compiler scalability. Avoid hot-loop copying and allocation first,
then use the [optimization catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) for measured
problems rather than making broad optimization work a prerequisite.

Port validation should use focused arithmetic/parser regressions, reference
scene comparisons and actual navigation. Compiler changes require their focused
goldens and the repository `make check` gate, plus applicable MSRV checks;
this documentation-only assessment uses `make docs-check` and diff hygiene.
