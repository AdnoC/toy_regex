#[derive(Debug)]
struct AStruct<'a> {
    a: i32,
    a_string_parameter: &'a str,
}

impl<'a> AStruct<'a> {
    fn new() -> AStruct<'a> {
        AStruct {
            a: 0,
            a_string_parameter: "Default",
        }
    }

    fn new_value_for_a(&mut self) {
        self.a = 4;
    }
}

fn main() {
    println!("Hello World!");
    let mut a = 43;
    println!("a is {}", a);
    do_thing(&mut a);
    println!("a is now {}", a);
    let a_struct = AStruct {
        a: 43,
        a_string_parameter: "Hello",
    };
    println!("Struct is {:?}", a_struct);
    let mut a_struct = AStruct::new();
    a_struct.new_value_for_a();
    do_other_thing(&a_struct);
    println!("Other struct is {:?}", a_struct);
}

fn do_thing(s: &mut i32) {
    *s *= 2;
}

/// # Hello world
/// These are doc comments
fn do_other_thing(s: &AStruct) {
    println!("Str is {}", s.a_string_parameter);
    let st = String::new();
    println!("empty string {}", st);
}
