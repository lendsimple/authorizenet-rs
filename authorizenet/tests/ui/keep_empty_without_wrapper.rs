use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Doc {
    #[anet(keep_empty)]
    items: Vec<String>,
}

fn main() {}
