extern crate rustlibxml;

use rustlibxml::XmlDoc;

#[test]
fn duplicate_file() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    doc.save_file("tests/results/copy.xml").unwrap();
}


