# Standard-Library Third-Party Notices

## Ryū binary64 conversion

`std/std/str/format_f64.ska` contains a Skald port of the finite binary64
conversion algorithm and size-optimized cached-power constants from
[Ryū revision `4c0618b0e44f7ef027ebae05d2cc7812048f7c8f`](https://github.com/ulfjack/ryu/tree/4c0618b0e44f7ef027ebae05d2cc7812048f7c8f),
principally `ryu/d2s.c`, `ryu/d2s_intrinsics.h`, `ryu/d2s_small_table.h`, and
`ryu/common.h`.

Copyright 2018 Ulf Adams.

The derived code is used under the Boost Software License 1.0, included at
[licenses/BSL-1.0.txt](licenses/BSL-1.0.txt). The port replaces C intrinsics
with portable 32-bit-limb multiplication, packs the size-optimized constants
as a canonical little-endian immortal string, lazily decodes one reusable
104-word table, and applies Skald's existing textual presentation contract.
