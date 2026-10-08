use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Doc {
    #[anet(attribute, wrapper, item = "x")]
    items: Vec<String>,
}

fn main() {}
