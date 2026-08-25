# General Iteration

Status: frozen source-visible design. The current compiler does not yet accept
the syntax or provide the canonical standard-library interface described here;
the [status matrix](STATUS.md) records implementation maturity.

This document is authoritative for general iteration semantics selected for
implementation. The frozen syntax extension is recorded in the
[grammar](GRAMMAR.md#frozen-general-iteration-extension), and compiler
representation is owned by the
[general-iteration compiler contract](../compiler/ITERATION.md).

## Canonical protocol

General iteration uses one nominal, state-based generic interface declared by
ordinary source in the canonical `std::iter` module:

```ska
public interface Iterable<Item, State> {
    fn iter_state() -> State;
    fn iter_next(mut ref state: State) -> Item?;
}
```

The compiler recognizes the exact module, interface name, generic arity,
requirement names, parameter modes, and substituted signatures. A declaration
with the same spelling elsewhere is an unrelated ordinary interface. A
missing or malformed canonical declaration is diagnosed rather than silently
replaced by compiler magic.

`Iterable` is library-declared but compiler-recognized because `for-in` must
select exact requirements and types without structural method discovery.
Neither the interface nor iteration requires a heap-allocated iterator.
`State` is an ordinary owning value and may itself be a primitive, class,
array, optional, or shared owner supported by ordinary stored-value rules.

## Source form and binding

The statement forms are:

```ska
for (item in expression) {
    statements
}

for (item: ItemType in expression) {
    statements
}
```

The parentheses and body block are mandatory. The loop is a statement and
produces no value. The item is a fresh immutable owning local in the body
scope; mutable or borrowed item bindings and destructuring patterns are not
part of this contract. The iterable expression resolves in the enclosing
scope, while the item is visible only inside the body.

`for` becomes a reserved keyword when the extension is implemented. `in` is
contextual only at the delimiter position in a `for` header and remains an
ordinary identifier elsewhere.

## Iterable selection

The static type of the iterable expression must provide exactly one eligible
closed application of `std::iter::Iterable<Item, State>`. Eligibility is
nominal and includes ordinary exact-class claims, inherited claims, specialized
generic-class claims, and a generic type parameter's exact interface bound.
Same-named methods do not make a value iterable.

Without an item annotation, exactly one eligible application must exist. With
`item: ItemType`, the annotation is an exact filter over candidate `Item`
types; it does not request a conversion, optional unwrap, shared dereference,
or best-match choice. No remaining candidate and multiple remaining candidates
are both errors.

For a callable body owned by a generic class, selection through a
type-parameter bound is fixed at definition-site checking. Specialization
substitutes the already selected requirement and closed interface identities;
it does not repeat overload-like selection against additional claims of the
concrete argument.

## Execution and termination

The iterable expression is evaluated exactly once. Before the first iteration
attempt, the compiler retains one read-only interface receiver for the entire
loop and secures any owner, checked-view guard, shared anchor, or array backing
anchor required to keep that receiver valid. It then calls `iter_state`
exactly once and stores the returned value in one hidden owning state.

Each iteration attempt performs these actions in order:

1. call `iter_next(mut ref state)` exactly once;
2. store its `Item?` result;
3. if the outer optional is absent, clean the result, state, and retained
   receiver resources and leave the loop;
4. if present, move or copy the payload according to ordinary optional and
   stored-value rules into a fresh owning item binding;
5. clean the consumed result wrapper and execute the body.

There is no prefetch, duplicate call, implicit length query, or call after the
first absent result. `iter_state`, every `iter_next`, item construction, body
effects, and cleanup therefore retain deterministic source order.

The outer optional layer alone reports termination. Consequently an optional
item retains two distinct levels:

```ska
return none;        // end iteration
return some(none);  // yield one absent optional item
return some(42);    // yield one present optional item
```

For `Item = T?`, the protocol result is `T??`; no layer is flattened or used
as an item sentinel.

## Lifetimes, exits, and mutation

Every entered iteration creates a fresh dynamic lifetime for the item and
ordinary body locals. Normal body completion and `continue` clean body locals
and the item before the next `iter_next` call. `break` cleans the same values
and then the hidden state and retained receiver resources before continuing
after the loop. `return` additionally performs the ordinary cleanup for every
exited enclosing function scope. Unrecoverable panic remains non-unwinding.

Unlabeled `break` and `continue` select the nearest enclosing `while` or
`for-in` loop through the same lexical loop-identity rule. Labels, loop values,
value-carrying exits, and loop `else` remain excluded. A `for-in` statement
conservatively has a termination fallthrough path for definite-return analysis.

The retained receiver is read-only and non-exclusive. The body may mutate
independent reachable state through ordinary legal aliases, including mutation
that affects later `iter_next` calls. Existing alias, final-field, shared-owner,
checked-view, and array-anchor restrictions remain authoritative; iteration
does not add snapshot, fail-fast, or concurrent-modification semantics.

## Standard-library adoption and cost

The canonical module will be dependency-free so foundational collection
modules may import it. `std::vec::Vec<T>` will ordinarily claim
`Iterable<T, u64>`: `iter_state` returns index zero and `iter_next` checks the
current length, produces the indexed value, and advances the state. Its state
and loop bookkeeping remain inline and allocation-free under the language
model. Item production follows `Vec<T>`'s ordinary copy/capability rules.

Allocation-free state is a semantic possibility, not a promise that every
dispatch is devirtualized or every loop is inlined. Devirtualization, range
fast paths, and vectorization are later optimizations that must preserve the
observable call and cleanup order above.

## Deliberate exclusions and extensions

The initial contract does not include operator overloading, numeric ranges or
`..` syntax, compiler-provided primitive or array conformances, structural
iteration methods, shared iterator objects, generators, consuming or mutable
receivers, borrowed items, adapters, patterns, or guaranteed optimization.

These are extension points rather than competing loop protocols. A future
`Range<T>` may implement this same `Iterable<T, T>` interface, and range syntax
may construct that value before ordinary `for-in` selection. Generator work
may choose a state that owns a resumable frame. Neither extension changes the
general loop contract.

The frozen rationale and rejected alternatives are retained in the
[design proposal](../roadmaps/GENERAL_ITERATION_DESIGN_PROPOSAL.md), and the
ordered delivery work is tracked in the
[implementation roadmap](../roadmaps/GENERAL_ITERATION_ROADMAP.md).
