use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Doc {
    #[anet(max_occurs = 3)]
    name: Option<String>,
}

fn main() {}
