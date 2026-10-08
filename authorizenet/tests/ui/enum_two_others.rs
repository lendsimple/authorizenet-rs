use authorizenet::xml::AnetEnum;

#[derive(AnetEnum)]
enum Color {
    #[anet(other)]
    First(String),
    #[anet(other)]
    Second(String),
}

fn main() {}
