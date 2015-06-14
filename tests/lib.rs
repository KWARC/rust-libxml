//! A few random tests.
//! Knowing how much I neglect this aspect of software development,
//! there probably won't be a significant coverage.

#![feature(hash)]
extern crate rustlibxml;

use rustlibxml::tree::{XmlDoc, XmlNodeRef};
use rustlibxml::xpath::{XmlXPathContext};
use rustlibxml::parser::xml_cleanup_parser;
use std::hash::{hash, SipHasher};

#[test]
/// Duplicate an xml file
fn duplicate_file() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    doc.save_file("tests/results/copy.xml").unwrap();
    xml_cleanup_parser();
}

#[test]
/// Can load an HTML file
fn can_load_html_file() {
    let doc = XmlDoc::parse_html_file("tests/resources/example.html").unwrap();
    assert_eq!(doc.get_root_element().unwrap().get_name(),"html");
    xml_cleanup_parser();
}

#[test]
/// Root node and first child of root node have different hash values.
/// (There is a tiny chance this might fail for a correct program)
fn child_of_root_has_different_hash() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    let root = doc.get_root_element().unwrap();
    assert!(!root.is_text_node());
    if let Some(child) = root.get_first_child() {
        assert!(root != child);
        assert!((hash::<XmlNodeRef, SipHasher>(&root)) !=
                (hash::<XmlNodeRef, SipHasher>(&child)));
//        assert!((hash::<XmlNodeRef, SipHasher>(&root)) != hash(&child));
    } else {
        assert!(false);   //test failed - child doesn't exist
    }
    xml_cleanup_parser();
}


#[test]
/// Test the evaluation of an xpath expression yields the correct number of nodes
fn test_xpath_result_number_correct() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    let context = XmlXPathContext::new(&doc).unwrap();
    let result1 = context.evaluate("//child").unwrap();
    assert_eq!(result1.get_number_of_nodes(), 2);
    assert_eq!(result1.get_nodes_as_vec().len(), 2);
    let result2 = context.evaluate("//nonexistent").unwrap();
    assert_eq!(result2.get_number_of_nodes(), 0);
    assert_eq!(result2.get_nodes_as_vec().len(), 0);
    xml_cleanup_parser();
}
