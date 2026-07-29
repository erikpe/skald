# Samples

Small Skald programs used for language bring-up and manual experimentation belong here. Regression assertions belong under `tests/`.

- `vertical/exit_42.ska` is the minimal original exit-status slice.
- `inline_counter.ska` demonstrates direct inline construction, mutable and
  read-only receiver methods, field access, and runtime output.
- `deterministic_destruction.ska` demonstrates automatic local and contained-
  field cleanup, including user-body-before-field and reverse-order semantics.
- `object_value_semantics.ska` demonstrates copy construction and assignment,
  internal object parameters/results, bounded produced-object temporaries,
  deterministic cleanup, and the direct constructor-elision cases.
- `polymorphism.ska` demonstrates inheritance, virtual and interface dispatch,
  an `Obj` type test, and checked object casts.
- `private_factory.ska` demonstrates a public static factory selecting a
  private ordinary initializer through exact-class lexical ownership.
- `strings.ska` demonstrates raw-byte literals, checked observation,
  shared-backing slices, copying construction from mutable bytes, independent
  byte-array conversion, and concatenation through the canonical standard
  library.
