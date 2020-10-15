use struct_gen::generate;

generate!(
  <T, G> where T: Sized, G: Copy {
    foo: T,
    bar: G
  } => {
    OnlyBar => [omit(foo)],
    OnlyFoo => [omit(bar)],
  }
);
