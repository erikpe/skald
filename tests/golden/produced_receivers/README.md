# Produced receiver golden tests

`receivers.golden.toml` owns complete source-to-native and source-to-diagnostic
coverage for read-only method calls on produced exact-class values. The native
matrix includes one compact source showing literal, construction, direct-call,
method-chain, generic-result, inherited, and virtual forms together. Its broad
stress case combines exact, inherited, virtual, and interface selection; every
call-result producer family; recursive register/stack pressure; raw-byte
strings; closed generic results; and owning-result lifetime observations in
one compilation unit. The compile-failure case keeps mutable methods and
excluded optional, array, and raw-shared receiver families at frontend
diagnostics. Produced field resolution is covered by focused resolver tests;
its complete executable matrix belongs to the produced-field roadmap.

Run this group, including repeated compiler and native processes, with:

```text
scripts/golden.sh --determinism full --filter 'produced_receivers/**'
```
