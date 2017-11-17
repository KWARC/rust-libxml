/// XML parsing module
pub mod xml;

use ergo::xml::{XmlInput, XmlError, ParseOptions};
use ergo::xml::document::Document;




/*
/// Convenience
pub fn XML<R: Read>(mut r: R) -> Result<Document, Vec<xml::XmlError> {
    let mut xml_str = String::new();
    r.read_to_string(&mut xml_str).expect("Could not read_to_string");
    xml::parse_string(&xml_str)
}
*/

pub fn xml_with_options<R: XmlInput + ?Sized>(r:&R, url: &str, encoding: &str, options: ParseOptions) -> Result<Document, Vec<XmlError>> {
    Document::parse(r, url, encoding, options)
}

pub fn xml<R: XmlInput + ?Sized>(r:&R) -> Result<Document, Vec<XmlError>> {
    xml_with_options(r, "", "utf-8", ParseOptions::DEFAULT_XML)
}


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
