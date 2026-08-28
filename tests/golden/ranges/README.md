# Range fixtures

These fixtures cover the canonical successor protocol, ordinary generic range
class, and concise-range frontend. Native coverage proves explicit primitive
and class iteration without a new runtime service. The concise fixture pins
the intentional typed-HIR gate that remains until concise construction
lowering lands.

Run this group with `scripts/golden.sh --filter 'ranges/**'`.
