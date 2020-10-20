use structout::generate;

generate!(
  <> {
    foo: u32,
    bar: u64
  } => {
    WithoutFoo => [only(bar)],
    WithoutBar => [only(foo)],
  }
);
