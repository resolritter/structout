use structout::generate;

generate!(
  <S, C, O> where S: Sized, C: Copy {
    foo: S,
    bar: C,
    other: i32
  } => {
    OnlyBar => [omit(foo), as_tuple()],
    OnlyFoo => [omit(bar), as_tuple()],
  }
);
