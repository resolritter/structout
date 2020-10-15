# struct_gen

# Usage

This library allows for generating multiple structs from a single definition through a procedural macro.

```
generate!(
  <...> where ... {
    field: type,
    ...
  } => {
    OutputStruct => [action(arg), ...]
  }
)
```

- `<...>` represents the type arguments
- `where ...` represents the type constraints
- `{ field: type, ... }` is the common *struct body* which will be used for generating new structs.
- `{ OutputStruct => [action(arg), ...] }` is the output configuration, where each entry maps to one new struct being generated; further:
   - `OutputStruct` is the name of the struct
   - `[action(arg), ...]` are the list of actions which will be used to build this specific variant.

Where "actions" can be one of:

- `omit(fields)` omits the fields from this struct definition
- `attr(attrs)` inserts an attribute before the struct definition

Put into practice:

```rust
use struct_gen::generate;

generate!(
  <> {
    foo: u32,
    bar: u64,
    baz: String
  } => {
    WithoutFoo => [omit(foo)],
    WithoutBar => [omit(bar)],
  }
);
```

The code above should expand to two structs

```rust
struct WithoutFoo {
    bar: u64,
    baz: String
}
struct WithoutBar {
    foo: u32,
    baz: String
}
```

---

If one were to add two generic arguments, they should be efficiently split between the variants without the need for PhantomData.

```rust
generate!(
  <S, C> where S: Sized, C: Copy {
    foo: S,
    bar: G
  } => {
    OnlyBar => [omit(foo)],
    OnlyFoo => [omit(bar)],
  }
);
```

The above code should expand to

```rust
struct OnlyBar<C>
where
    C: Copy,
{
    bar: G,
}
struct OnlyFoo<S>
where
    S: Sized,
{
    foo: S,
}
```

For examples of usage for the full API, consult the [tests module](./src/lib.rs).

# Development

## Testing

Testing revolves around snapshot testing ([insta](https://crates.io/crates/insta)). It's effectively done by running `cargo expand` ([cargo-expand](
https://crates.io/crates/cargo-expand)), getting its output, then reviewing with it with `cargo insta review` ([cargo insta](https://crates.io/crates/cargo-insta)).

Consult the [tests module](./src/lib.rs) for seeing how it's implemented in practice.

# Motivation

This library attends to the need of generating multiple structs for a single definition. Consider the code

```rust
struct Human {
  id: u32,
  age: u32,
  username: String,
  name: String,
  surname: String
}

// suppose this is what you would get from an API
struct HumanEditableParts {
  name: String,
  surname: String
}
```

`HumanEditableParts` manually repeats some of the fields and those need to be kept in sync.

In Rust, it is said this pattern can be avoided with "struct composition". i.e.

```rust
struct HumanEditableParts {
  name: String,
  surname: String1
}

struct Human {
  id: u32,
  age: u32,
  username: String,
  editable_parts: HumanEditableParts
}
```

However, that is not always feasible, nor always pleasant to do.
