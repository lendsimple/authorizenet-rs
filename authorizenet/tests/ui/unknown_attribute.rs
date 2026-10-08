use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
struct Doc {
    #[anet(renam = "x")]
    field: Option<String>,
}

fn main() {}
