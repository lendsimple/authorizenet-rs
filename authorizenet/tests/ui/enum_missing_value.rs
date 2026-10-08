use authorizenet::xml::AnetEnum;

#[derive(AnetEnum)]
enum Color {
    #[anet(value = "red")]
    Red,
    Blue,
}

fn main() {}
