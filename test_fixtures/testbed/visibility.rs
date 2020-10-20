use structout::generate;

generate!(
  pub(crate) <> {
    foo: u32,
  } => {
    Everything => [],
  }
);
