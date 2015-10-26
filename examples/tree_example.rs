extern crate libxml;

use libxml::tree::*;
use libxml::parser::*;


fn my_recurse(node : &XmlNodeRef) {
    match node.get_type().unwrap() {
       XmlElementType::ElementNode => {
           println!("Entering {}", node.get_name());
       }
       XmlElementType::TextNode => {
           println!("Text: {}", node.get_content());
       }
        _ => { }
    }

    let mut c : Option<XmlNodeRef> = node.get_first_child();
    loop {
        match c {
            Some(child) => {
                my_recurse(&child);
                c = child.get_next_sibling();
            },
            None => break,
        }
    }

    if node.get_type().unwrap() == XmlElementType::ElementNode {
        println!("Leaving {}", node.get_name());
    }
}

fn main() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    let root = doc.get_root_element().unwrap();
    my_recurse(&root);
    xml_cleanup_parser();
}
