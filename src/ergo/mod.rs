/// XML parsing module
pub mod xml;

use ergo::xml::{Document, XmlInput, XmlError, ParseOptions};


/*
/// Convenience
pub fn XML<R: Read>(mut r: R) -> Result<Document, Vec<xml::XmlError> {
    let mut xml_str = String::new();
    r.read_to_string(&mut xml_str).expect("Could not read_to_string");
    xml::parse_string(&xml_str)
}
*/

pub fn xml_with_options<R: XmlInput + ?Sized>(r:&R, url: &str, encoding: &str, options: ParseOptions) -> Result<Document, Vec<XmlError>> {
    Document::parse_with_options(r, url, encoding, options)
}

pub fn xml<R: XmlInput + ?Sized>(r:&R) -> Result<Document, Vec<XmlError>> {
    Document::parse(r)
}

/*
pub fn fragment(xml_str: &str) -> NodeSet {
    panic!("Not implemented")
}
*/
#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::Path;
    use super::*;
    #[test]
    fn ergo_test(){
        assert!(xml("<root></root>").is_ok());
        assert!(xml(&String::from("<root></root>")).is_ok());
        assert!(xml(&File::open("tests/resources/file01.xml").unwrap()).is_ok());
        assert!(xml(Path::new("tests/resources/file01.xml")).is_ok());
    }
}
