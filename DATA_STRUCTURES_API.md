# Data Structures API Proposal (Stage-0/Stage-1 Friendly)

This proposal defines a practical path to vectors and maps with the current compiler/runtime capabilities.

## Constraints (Current Compiler)

- No language-level generics yet.
- No function pointers/callbacks.
- No pointer casts.
- Manual memory management is explicit; `defer` is available and should be used for cleanup.

Because of this, the most practical design is:

1. **Typed containers** in the user-visible API (`VecI64`, `VecPair`, `MapU64Pair`, etc.).
2. **Template/codegen-generated wrappers** for each concrete type.
3. Shared C runtime internals for growth, probing, hashing, copy.

---

## Ownership Model (Simple + Safe)

For every data structure instance:

- `init` / `create` allocates internal storage.
- `destroy` frees internal storage.
- User should register cleanup with `defer` right after successful init.

Pattern:

```toy
var v: VecI64 = vec_i64_new();
defer vec_i64_destroy(&v);
```

If a container stores pointers, the container owns only pointer slots, not pointees.

---

## Vector API (Typed)

## Visible Struct Shape

```toy
struct VecI64 {
  data: *i64;
  len: u64;
  cap: u64;
}
```

Equivalent typed variants can be generated for other element types:

- `VecU64`
- `VecBool`
- `VecPtrPair` (or `VecPairPtr` naming convention)
- `VecPair` (stores structs by value)

## Functions

```toy
extern fn vec_i64_new() -> VecI64;
extern fn vec_i64_destroy(v: *VecI64) -> unit;
extern fn vec_i64_reserve(v: *VecI64, min_cap: u64) -> bool;
extern fn vec_i64_push(v: *VecI64, value: i64) -> bool;
extern fn vec_i64_pop(v: *VecI64, out: *i64) -> bool;
extern fn vec_i64_get(v: *VecI64, idx: u64, out: *i64) -> bool;
extern fn vec_i64_set(v: *VecI64, idx: u64, value: i64) -> bool;
extern fn vec_i64_len(v: *VecI64) -> u64;
extern fn vec_i64_clear(v: *VecI64) -> unit;
```

Semantics:

- `bool` return indicates success (`false` on OOM / index error / empty pop).
- No panics/exceptions in MVP.

## Example

```toy
extern fn print_i64(x: i64) -> unit;

fn vec_demo() -> unit {
  var v: VecI64 = vec_i64_new();
  defer vec_i64_destroy(&v);

  var ok: bool = vec_i64_push(&v, 10);
  if !ok { return; }
  ok = vec_i64_push(&v, 20);
  if !ok { return; }

  var x: i64 = 0;
  ok = vec_i64_get(&v, 1, &x);
  if ok { print_i64(x); }
}
```

---

## Map API (Typed, `u64` key first)

Start with `u64` keys for simplicity and performance.

## Visible Struct Shape

```toy
struct MapU64I64 {
  keys: *u64;
  vals: *i64;
  states: *u8;
  len: u64;
  cap: u64;
}
```

`states` uses open-addressing metadata (empty/occupied/deleted).

Equivalent generated variants:

- `MapU64U64`
- `MapU64Pair`
- `MapU64PtrNode` (pointer values)

## Functions

```toy
extern fn map_u64_i64_new() -> MapU64I64;
extern fn map_u64_i64_destroy(m: *MapU64I64) -> unit;
extern fn map_u64_i64_reserve(m: *MapU64I64, min_cap: u64) -> bool;
extern fn map_u64_i64_put(m: *MapU64I64, key: u64, value: i64) -> bool;
extern fn map_u64_i64_get(m: *MapU64I64, key: u64, out: *i64) -> bool;
extern fn map_u64_i64_remove(m: *MapU64I64, key: u64, out: *i64) -> bool;
extern fn map_u64_i64_contains(m: *MapU64I64, key: u64) -> bool;
extern fn map_u64_i64_len(m: *MapU64I64) -> u64;
extern fn map_u64_i64_clear(m: *MapU64I64) -> unit;
```

## Iteration (No callbacks)

Because there are no function pointers, use index-based iteration:

```toy
extern fn map_u64_i64_iter_begin(m: *MapU64I64) -> u64;
extern fn map_u64_i64_iter_next(m: *MapU64I64, idx: u64) -> u64; // returns cap when done
extern fn map_u64_i64_iter_key(m: *MapU64I64, idx: u64) -> u64;
extern fn map_u64_i64_iter_val(m: *MapU64I64, idx: u64) -> i64;
extern fn map_u64_i64_cap(m: *MapU64I64) -> u64;
```

---

## Arbitrary Data Support Strategy

You can support numbers, pointers, and structs by **typed specialization**:

- Numbers: direct typed containers (`VecI64`, `MapU64I64`, etc.)
- Pointers: typed pointer containers (`VecPtrFoo`, `MapU64PtrFoo`)
- Structs by value: typed struct containers (`VecPair`, `MapU64Pair`)

This avoids casts and keeps type checking simple.

---

## How to Avoid Boilerplate (Without Full Generics)

Add a small generator script (`scripts/gen_ds_api.py`) that takes a list of concrete types and emits:

- C runtime wrappers (`runtime/ds_generated.c/.h`)
- toy extern declarations (`stdlib/ds_generated.toy`)

Input example:

- `Vec: i64, Pair, *Node`
- `Map: u64->i64, u64->Pair, u64->*Node`

This gives a "generic-like" developer experience while keeping compiler complexity low.

---

## Why Not Full Generics Yet?

True generics require at least:

- Generic syntax and parsing
- Type-parameter checking/inference rules
- Monomorphization or dictionary passing
- Name mangling and specialization codegen

That is feasible later, but bigger than adding useful data structures now.

---

## Suggested Rollout

1. Implement `VecI64` + `MapU64I64` in runtime C.
2. Add typed struct variants used by tests (`VecPair`, `MapU64Pair`).
3. Add generator for wrappers/externs.
4. Add golden tests with `defer destroy(...)` patterns.
5. Optionally add language generics after runtime/container semantics are stable.

---

## MVP Developer Rules to Document

- Always pair `new/init` with `defer destroy` in same scope.
- Check boolean return from mutating operations.
- Containers own their internal buffers, not pointed-to heap payloads.
- For pointer payloads, user must free pointees explicitly (often via map/vector iteration).
