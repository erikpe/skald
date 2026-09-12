# Auric in Skald: feasibility assessment

Status: actionable investigation; no emulator implementation roadmap started.
Next step: select one firmware revision and TAP game, then validate CPU/bus
execution speed and the shared framebuffer bridge before expanding the port.

## Assessment

**A silent, mostly playable Oric emulator is feasible in Skald, and Auric is
probably the better first port target than Doom.** It has a much smaller retained
core, simple memory ownership, and an eight-bit workload well matched to types
Skald already supports. No new integer types or change to division semantics
is necessary to begin.

The qualification is accuracy: an emulator cannot approximate CPU semantics in
the same way a game port can simplify visual effects. Incorrect flags, stack
operations, interrupts or device registers can prevent guest software from
running at all. The appropriate simplification is a narrower supported machine
and software set, retaining the behavior that set actually uses.

Recommended first target:

- Linux x86-64, one Atmos firmware revision, one standard TAP game.
- CPU, memory bus, VIA timers/interrupts, keyboard matrix, and text/hires video
  implemented in Skald.
- A small native window/input/time/presentation library, reusing the proposed
  [Doom shared-array bridge](DOOM_PORT_FEASIBILITY.md#smaller-proof-of-concept-option-expose-the-shared-array-handle).
- Silent execution, no disk controller, debugger GUI, snapshots, save support,
  printer, shader effects, alternate machine models or custom tape loaders.
- Real-time guest execution and keyboard controls, tested through actual play.

This is still a multi-stage project. Several weeks to a few months is a plausible
planning range for one developer, depending heavily on compiler performance and
guest compatibility problems; this is judgment, not a measured delivery estimate.

## Evidence and scale

Inspected Auric revision `15c7222d6ebaf888deb70a5408a7db9721380590` on
2026-09-11. The sibling checkout had no local changes. Source paths below are
relative to `../Auric/`; it is outside this repository's documentation links.

Physical line counts include comments, declarations and optional functionality:

| Group | Files | Lines |
| --- | ---: | ---: |
| `src/chip/mos6502*`: CPU, opcodes, cycle table | 4 | 1,831 |
| `src/chip/mos6522*`: VIA | 2 | 1,081 |
| `src/chip/ula.*`: video | 2 | 269 |
| `src/chip/ay3_8912.*`: sound and register/I/O behavior | 2 | 1,000 |
| `src/machine.*`, `src/memory.*` | 4 | 1,125 |
| `src/tape/*.cpp`, `*.hpp` | 11 | 1,534 |
| Source excluding `third_party` and vendored `imgui` directories | 67 | 13,304 |
| Test C++ sources and headers | 13 | 4,544 |

The CPU/VIA/video subtotal is 3,181 lines. Tape and machine code need pruning;
most AY synthesis can disappear. These are scope indicators, not a predicted
Skald line count. Do not compare the repository's raw total to Doom: Auric also
vendors substantial GUI/OpenGL code that the proof of concept need not port.

Reviewed `README.md`, `ROMS/README_ROMS.md`, machine/bus wiring, CPU execution
and cycle tables, VIA execution/registers, AY register updates, ULA rendering,
SDL input, tape parser/turbo interception, ROM patches, and CPU/VIA/tape tests.
Skald's [status matrix](../language/STATUS.md),
[primitive semantics](../language/TYPES_AND_VALUES.md),
[arrays](../language/ARRAYS.md), [aliases](../language/ALIASES_AND_OWNERSHIP.md),
[I/O](../language/IO.md), [function values](../language/FUNCTION_VALUES.md),
and [backend](../compiler/BACKEND.md) provide the language baseline. The sibling
Niflheim Traci feasibility record was consulted for staged-port structure.

## Firmware and game loading are separate

Auric's ROMs are system firmware, not normally game cartridges. The repository
README describes BASIC ROMs for Oric 1/Atmos and a Microdisc ROM, then loads games
from `.tap` or disk images. A realistic acceptance target is **boot firmware,
load a game image, play it**, rather than copy an arbitrary game ROM into memory.
A custom standalone ROM is possible but requires its own memory/entry contract.

No `.rom`, `.tap` or `.dsk` files were found in this checkout. The ROM inventory
identifies Atmos `basic11b.rom` with SHA-1
`9451a1a09d8f75944dbd6f91193fc360f1de80ac`; Auric's turbo intercepts are explicitly
matched to firmware hashes. Select an actual image and game before claiming
compatibility. Screenshots, including Hunchback, are evidence of the project's
intended use, not a boot test performed here.

Prefer the read-only portion of Auric's ROM-aware turbo tape path:

1. Boot the selected firmware through its reset vector.
2. Issue the firmware's loading command through emulated keyboard input.
3. Recognize the selected ROM's sync/read-byte routines and feed TAP bytes,
   preserving the CPU flags, scratch-memory writes and continuation PCs in
   `tape_tap_turbo.cpp` and `rom_patcher.cpp`.
4. Leave the normal firmware code responsible for the remaining loading flow.

The existing turbo class inherits the normal tape implementation and includes
saving/fallback paths; do not assume copying it verbatim creates a small loader.
Extract read-only sync, parsing and byte interception. Reject unsupported media
or loader behavior explicitly. Validate headers, bounded NUL-terminated names,
start/end addresses, body lengths and multi-block progression. TAP header
addresses in this parser use high-byte-first encoding; CPU words use
little-endian encoding. Use separate helpers.

A direct RAM injection plus PC jump can bootstrap a specially selected machine
code program, but does not generally recreate BASIC state, tape headers,
autostart behavior, or custom loaders. It is a useful test harness, not the
preferred general game-loading contract. Defer cassette waveform emulation,
custom fast loaders, saving, disk games and overlays until a chosen game needs
them. Turbo intercepts must remain Skald emulator logic, not native-library work.

## Retained machine behavior

### CPU and memory bus

Auric allocates 64 KiB RAM, a 16 KiB BASIC ROM, and an optional 8 KiB disk ROM.
With the normal ROM enabled, reads at `0xc000` and above select firmware and
writes are ignored. The `0x0300` page routes to devices. Zero-page and stack
access have specialized paths. Port the bus behavior, not just a flat RAM array.

There is a subtle no-disk boundary: current `Machine` intercepts
`0x0310..0x031b` for its drive object even with `DriveNone`, whose reads return
zero and writes do nothing. Make an explicit compatibility choice about this
range; blindly routing the entire page to VIA changes the template's behavior.
Initially preserve Auric's selected no-drive profile and cover it with bus tests.

The CPU implementation uses `time_instruction()` followed by `exec()`: despite
some cycle-oriented comments, execution dispatches whole instructions, while
machine scheduling accounts for their cycles. Preserve that ordering first.
Essential CPU contracts include:

- 8-bit result wrapping; carry/borrow and overflow calculated before narrowing.
- 16-bit PC/address wrapping, signed branch offsets, zero-page wrap and stack
  page semantics.
- Page-crossing and branch cycle adjustments; reset/IRQ/NMI entry and RTI/RTS.
- Decimal ADC/SBC, status-register behavior and the indirect-JMP page-wrap case.
- Self-modifying RAM code and actual mapped read/write side effects.

Auric implements several undocumented instructions and has unhandled cases;
its cycle table contains zero entries. Initially support its useful opcode set,
but make unsupported execution a clear diagnostic/halt rather than allowing
zero-cycle loops. Do not assume undocumented instructions are dispensable for
every game. Validate decimal/interrupt behavior independently when required;
source comments and comparisons to this one emulator do not prove silicon
accuracy. This assessment does not certify or redesign Auric's CPU semantics.

### Silent AY behavior and keyboard scanning

**Removing the AY chip entirely would break keyboard input.** In this code:

- VIA port A carries AY register data.
- CA2 and CB2 control AY BC1/BDIR and register selection/writing.
- VIA port B bits 0–2 select one of eight keyboard rows.
- `Machine::update_key_output()` checks AY `ENABLE` bit `0x40` and the inverted
  `IO_PORT_A` register to select key columns, then sets VIA input bit 3.

Keep a small AY register file, selected-register state, control-line updates,
and the keyboard-related port behavior. Store non-audio-visible state needed
by guest writes; omit tone/noise/envelope generation, audio queues, locks,
callbacks and host audio setup. This is silent chip I/O emulation, not sound
synthesis. Auric explicitly leaves the PSG bus-read case unimplemented, so
support beyond its current path must be driven by a selected game/test.

Host input should deliver press/release events, then Skald maps keys to the
Oric matrix and maintains simultaneous held keys. Include modifiers and clear
held state on focus loss. Keep this separate from text input: games scan keys,
not Unicode characters. No C-to-Skald callbacks are necessary.

### VIA, timing and interrupts

Do not remove VIA timers merely because sound is absent. They also implement
guest timing and interrupt behavior. Preserve ports, direction/control
registers, T1/T2, interrupt flags/enables, clear-on-read behavior and the control
signals used by AY/tape. Other modes, including shift-register behavior, can be
deferred only after confirming the selected software does not use them.

Auric's loop budgets 64 cycles per raster, with 312 rasters per frame, and paces
a frame every 20 ms: **19,968 guest cycles/frame and 998,400 cycles/second**.
This is derived from the inspected implementation, not a new hardware timing
specification. It carries instruction overshoot between raster budgets; keep a
signed cycle balance. VIA currently iterates per guest cycle within each
instruction budget. Preserve timer/interrupt ordering before attempting batching.

Start with Auric's fixed 50 Hz profile. Retain raster-by-raster video updates
because guest writes can change display state during a frame. Dropping host
presentation frames under load is preferable to arbitrarily deleting guest
instructions or timer work. Bound host catch-up after a stall and keep input
polling responsive. Host pacing and guest cycle accounting are different jobs.

### Video

ULA produces a **240×224, four-byte-per-pixel** buffer: 215,040 bytes/frame,
about 10.75 MB/s at 50 presentations/second. This is modest transfer volume,
although the Skald bridge and rendering cost still need measurement.

Keep text and hires modes, the bottom text area, writable character data,
serial ink/paper/video attributes, alternate/double-height characters,
blinking and inverse colors. These are part of the game's data format and
appearance, not optional shader effects. The core renderer expands 40 groups
of six pixels per raster; it is far smaller than Doom's scene renderer.

Replace `reinterpret_cast` pixel stores and packed mask tricks with explicit
RGBA byte writes first. Existing `u64` can represent packed colors if later
useful; no `u32` or unsafe pointer support is required. All ULA decoding and
color selection stays in Skald. C receives finished pixels and only scales,
uploads and presents them. Drop OpenGL scanline effects, ImGui windows, status
bars and font assets used by the host UI.

## Skald representation and language needs

Use one owning machine aggregate with CPU/VIA/AY/video state and byte arrays,
plus explicit helper calls accepting aliases. Avoid translating C++ back
references (`Machine&` held by chips) into recursive inline containment or
strong ownership cycles. Chip state need not store its parent: a machine-level
step function can coordinate it, and bus functions can take the state they need.
Memory reads can mutate device state; give bus APIs the appropriate mutable
access. Avoid accessing a copied machine or copying 64 KiB of RAM per call.

| Concern | Initial Skald solution | New feature required? |
| --- | --- | --- |
| CPU registers/RAM/ROM | `u8`, `u8[]`, explicit casts | No |
| PC, addresses and 16-bit timers | `u64` or `i64` with explicit `& 0xffff` at hardware boundaries | No; `u16` would be convenience |
| Carry/borrow intermediates | Widen before addition/subtraction, narrow the result afterward | No |
| Branch displacement | `(i64) byte`, subtract 256 when bit 7 is set | No |
| Instruction/cycle tables | Byte/integer arrays and grouped opcode dispatch | No |
| C++ switch statements | Small grouped `if`/`elif` dispatch helpers | No; integer `match`/`switch` would help |
| Bus/device callbacks | Direct functions for one machine; typed internal callbacks if justified | No closures or foreign callbacks needed |
| Binary assets | Existing binary-safe `read_file`, then byte storage | No seek/write/filesystem API needed |
| Pixel transfer | Proposed borrowed shared-array handle bridge | Small compiler bridge still needed |
| Host input/time | Primitive-returning extern wrappers | Existing interop suffices |

The arithmetic trap differs from Doom: C++ promotes byte operands before many
operations; Skald's exact `u8 + u8` wraps immediately. For ADC, widen A/value/carry
before calculating carry and overflow. Shifting a `u8` high byte by eight also
fails Skald's shift bounds: cast to a wide type first, and use a `u64` shift count.
Similarly, a 16-bit timer decrement represented in `u64` must wrap at 16 bits,
not 64. Make these operations explicit, tested helpers.

No `i32`/`u32`, float math library, truncating signed division extension, raw
pointers, unions, weak references or threads are prerequisites. Numeric opcode
dispatch is the most relevant language convenience: a linear chain across all
opcodes could be expensive. Group by high nibble or operation/addressing family
to bound dispatch depth, then measure. A proper integer switch with efficient
lowering would help future interpreters too, but should not delay the first
CPU prototype. Generated tables/code can reduce transcription errors.

The shared-array native adapter can use the same version-pinned byte layout as
Doom. That approach still needs compiler-supported passage of a borrowed handle;
C decoding the header alone does not bypass current external-signature checks.
Use synchronous consumption, keep the owner alive, and centralize offsets in
one native function. CPU, tape and memory operations must not cross into C on
every instruction/access. Emit assembly and link the native shim/runtime through
the existing host toolchain, as described in the Doom assessment.

## Performance and correctness gates

The small emulated machine is encouraging, but there is no measured Skald
emulator throughput yet. Around one million guest cycles per second entails
many more host operations, especially VIA per-cycle work, bounds checks,
state loads and dispatch. Do not infer playability from framebuffer bandwidth.

Measure a CPU/bus prototype on a named machine with synthetic instruction mixes
and hot RAM/I/O paths, followed by VIA and ULA. Compare guest cycles retired per
host second and time for a full 19,968-cycle frame. Use preallocated state and
avoid allocation, deep copies or host calls in the hot loop. Test normal and
omitted runtime tracing when measuring; preserve correctness checks.

Auric supplies 64 CPU test declarations, concentrated on load/add/subtract
families; it is not a complete opcode/addressing/interrupt conformance suite.
The five VIA test files contain 101 test declarations, and turbo tape has 11.
These counts describe source tests, not tests executed by this investigation.
Port useful fixtures and expected behavior, then add missing branch, stack,
interrupt, bus-boundary and timer cases. CPU differential traces against Auric
are valuable for translation errors, but shared bugs require independent
hardware-conformance evidence if a game exposes them.

Suggested acceptance milestones (split into PR-sized tasks later):

1. **CPU/bus spike:** synthetic programs pass register/flag/memory/cycle checks;
   all wrap boundaries behave deliberately; measured execution has enough budget
   for remaining devices. Establish a bounded instruction limit for test hangs.
2. **Firmware boot and video:** reset through the real ROM vector, render the
   BASIC startup screen, and compare text/hires raster fixtures to Auric.
3. **VIA/AY keyboard:** type BASIC commands through actual matrix scanning;
   verify timer interrupts, held/released keys and modifier combinations.
4. **Read-only TAP loading:** run the selected firmware-aware loader against one
   validated tape image; verify the game starts. A BASIC prompt alone is not
   game compatibility.
5. **Playable session:** title/menu controls, start, movement/actions, failure
   and restart, sustained guest speed and repeated input without stuck keys.
   Aim for full guest time with 50 Hz presentation, allowing lower presentation
   rate if gameplay remains correctly paced. Record supported image hashes and
   exclusions before trying a second game.

Disk support, custom loaders and timing-sensitive demos should remain outside
this first acceptance set. The inspected repository has no bundled firmware or
game media and this investigation did not build/run Auric, implement an emulator,
or benchmark native Skald CPU execution. ROM/game compatibility is unverified.

## Comparison with Doom

| Dimension | Auric proof of concept | Doom proof of concept |
| --- | --- | --- |
| Retained core | Small CPU/VIA/ULA and machine/loader wiring | Much larger renderer, world, collision and map-special machinery |
| Data ownership | Fixed RAM, small chip state, a few asset buffers | Interconnected map/entity graphs and caches |
| Numeric fit | Existing `u8` plus masked addresses fits well | Signed 32-bit fixed point, wrapping angles, C division differences |
| Main correctness risk | Guest instructions, I/O, interrupts and timing | Geometry, clipping, movement and level logic |
| Removing sound | Retain AY register/I/O subset for keys | Remove audio and choose AI-alert behavior separately |
| Content dependency | Firmware plus selected tape game | Selected IWAD and maps |
| Best early proof | Boot BASIC, scan keyboard, then load game | Load map, render view, then navigate |

**Recommendation:** use Auric first if the goal is an achievable Skald systems
proof of concept. It exercises byte arithmetic, mutable state, dispatch,
arrays and native presentation with fewer engine subsystems to rewrite. Build
the presentation bridge so Doom can reuse it later. Choose Doom first only if
the game-port workload itself is the priority; Auric success does not establish
Doom performance or compiler scalability.

Documentation validation: `make docs-check` and `git diff --check`. Future
compiler changes require focused goldens and `make check`, with applicable MSRV
checks; emulator changes need the execution and compatibility gates above.
