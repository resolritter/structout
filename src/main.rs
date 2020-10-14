use lib::defn;

defn!(<T> { foo: u32, bar: T } => { OnlyBoo => [omit("bar")] });

fn main() {
    println!("Hello!");
}
