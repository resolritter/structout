use structout::generate;

generate!(
  <> {
    foo: u32,
  } => {
    NewFields => [upsert(bar: i32, baz: i64)],
    OverriddenField => [upsert(foo: u64)],
    Tupled => [as_tuple(), upsert(foo: u64)]
  }
);
