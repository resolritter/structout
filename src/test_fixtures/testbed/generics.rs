use struct_gen::gen_generics;

gen_generics!(
  <T> {
    foo: u32,
    bar: T
  } => {
    OnlyBar => [omit(foo)],
    OnlyFoo => [omit(bar)],
  }
);
