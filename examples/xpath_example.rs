extern crate rustlibxml;

use rustlibxml::tree::*;
use rustlibxml::parser::*;
use rustlibxml::xpath::XmlXPathContext;


fn main() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    let context = XmlXPathContext::new(&doc).unwrap();
    let result = context.evaluate("//child/text()").unwrap();

    for node in result.get_nodes_as_vec().iter() {
        println!("Found: {}", node.get_content());
    }

    xml_cleanup_parser();
}
