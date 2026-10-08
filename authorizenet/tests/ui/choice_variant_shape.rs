use authorizenet::xml::AnetXml;

#[derive(AnetXml)]
enum Pick {
    Card(String),
    Pair(String, String),
}

fn main() {}
