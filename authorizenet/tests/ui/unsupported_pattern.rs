use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Doc {
    #[anet(pattern = "[0-9]{3}")]
    code: Option<String>,
}

fn main() {}
