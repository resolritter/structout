use structout::generate;

generate!(
  <S, C> where S: Sized, C: Copy {
    foo: S,
    bar: C
  } => {
    OnlyBar => [omit(foo)],
    OnlyFoo => [omit(bar)],
  }
);
