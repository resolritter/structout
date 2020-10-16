use structout::generate;

generate!(
  <> {
    foo: u32,
    bar: u64,
    baz: String
  } => {
    WithoutFoo => [omit(foo)],
    WithoutBar => [omit(bar)],
    WithAttrs => [attr(#[object(context=Database)]), attr(#[object(config="latest")])],
  }
);
