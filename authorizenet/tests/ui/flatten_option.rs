use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Base {
    field: Option<String>,
}

#[derive(AnetXml)]
struct Doc {
    #[anet(flatten)]
    base: Option<Base>,
}

fn main() {}
