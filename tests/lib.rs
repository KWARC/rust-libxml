extern crate rustlibxml;

use rustlibxml::tree::XmlDoc;

#[test]
fn duplicate_file() {
    let doc = XmlDoc::parse_file("/tmp/f.xml").unwrap();
    doc.save_file("/tmp/g.xml").unwrap();
    //rustlibxml::xml_cleanup_parser();
}


