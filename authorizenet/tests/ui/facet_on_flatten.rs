use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Base {
    name: Option<String>,
}

#[derive(AnetXml)]
struct Doc {
    #[anet(flatten, max_length = 3)]
    base: Base,
}

fn main() {}
