use struct_gen::generate;

generate!(
  <T> {
    foo: u32,
    bar: T
  } => {
    OnlyBar => [omit(foo)],
    OnlyFoo => [omit(bar)],
  }
);
