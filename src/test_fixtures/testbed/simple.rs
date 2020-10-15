use struct_gen::generate;

generate!(
  <> {
    foo: u32,
    bar: u64,
    baz: String
  } => {
    OnlyBar => [omit(foo)],
    OnlyFoo => [omit(bar)],
    Everything => [],
  }
);
