# Standard language-interface golden tests

`interfaces.golden.toml` verifies that the dependency-free `std::lang`
interfaces can be selectively imported, implemented together, selected through
generic bounds, and dispatched natively. It also covers the `Equatable`
contract's unrelated-`Obj` false result and the `Hashable` `u64` result.

Run this group with `scripts/golden.sh --filter 'standard_lang/**'`.
