use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Doc {
    #[anet(wrapper)]
    items: Vec<String>,
}

fn main() {}
