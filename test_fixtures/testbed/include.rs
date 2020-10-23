use structout::generate;

generate!(
  {
    foo: u32,
    bar: u64
  } => {
    WithoutFoo => [include(bar)],
    WithoutBar => [include(foo)],
  }
);
