use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
#[anet(request)]
struct Doc {
    field: Option<String>,
}

fn main() {}
