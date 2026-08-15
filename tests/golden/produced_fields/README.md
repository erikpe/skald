# Produced field golden tests

`fields.golden.toml` owns the first executable produced-object field-read
slice. It covers a direct primitive load and nested inline-class fields used
as a method receiver, explicit-copy source, and read-only alias argument.

Run the group with:

```text
scripts/golden.sh --determinism full --filter 'produced_fields/**'
```
