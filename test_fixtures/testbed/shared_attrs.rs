use structout::generate;

generate!(
  #[derive(std::fmt::Debug)]
  {
    foo: u32,
  } => {
    InheritsAttributes => [],
    InheritsAttributesTwo => [],
  }
);
