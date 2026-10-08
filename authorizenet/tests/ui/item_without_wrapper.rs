use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Doc {
    #[anet(item = "x")]
    items: Vec<String>,
}

fn main() {}
