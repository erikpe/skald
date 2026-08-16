# `Str` Cached-Hash Migration Discovery

Status: resolved and archived by the completed cached-hash migration roadmap.

## Problem

`std::str::Str` can now use a `private cell` field for a cached hash, but the
compiler-known string descriptor is frozen to exactly three direct fields:
`_storage`, `_start`, and `_length`. Compiler-created literals initialize those
three identities directly. Adding `_hash_code: u64?` only in standard-library
source would therefore invalidate language-item resolution and leave intrinsic
literal materialization incomplete.

## Evidence and boundary

The exact descriptor is specified in
[Strings Compiler Contract](../compiler/STRINGS.md#canonical-language-item)
and enforced while resolving the string language item. HIR and MIR literal
metadata name exactly the storage, start, and length fields. Private cell
support deliberately changes none of those identities, representation rules,
or literal publication steps.

This is not a private-cell correctness gap. It is a coordinated string
language-item representation migration and must preserve deterministic literal
construction, synthesized lifecycle, dynamic string creation, slicing, layout,
and runtime ABI version 9.

## Follow-up

Owner: a future string language-item roadmap.

Priority: normal, after a standard-library map or other measured consumer
demonstrates that cached string hashing is worthwhile.

The roadmap should decide the cache's zero/absence representation, extend
language-item validation and all literal field metadata atomically, initialize
compiler-created literals consistently, update `std::str::Str`, and add phase,
native, determinism, malformed-language-item, and ABI/layout regressions.
