//! A few random tests.
//! Knowing how much I neglect this aspect of software development,
//! there probably won't be a significant coverage.

#![feature(hash)]
extern crate rustlibxml;

use rustlibxml::tree::{XmlDoc, XmlNodeRef};
use rustlibxml::xpath::{XmlXPathContext};
use rustlibxml::parser::xml_cleanup_parser;
use std::hash::{Hash, Hasher, SipHasher};
use std::fs::File;
use std::io::Read;

fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = SipHasher::new();
    t.hash(&mut s);
    s.finish()
}

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
/// Can parse an xml string in memory
fn can_parse_xml_string() {
    let mut file = File::open("tests/resources/file01.xml").unwrap();
    let mut xml_string = String::new();
    file.read_to_string(&mut xml_string).unwrap();
    let doc = XmlDoc::parse_xml_string(&xml_string).unwrap();
    assert_eq!(doc.get_root_element().unwrap().get_name(), "root");
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
        assert!((hash(&root)) != (hash(&child)));
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


#[test]
/// Test that an xpath expression finds the correct node and
/// that the class names are interpreted correctly.
fn test_class_names() {
    let doc = XmlDoc::parse_html_file("tests/resources/file02.xml").unwrap();
    let context = XmlXPathContext::new(&doc).unwrap();
    let result = context.evaluate("/html/body/p").unwrap();
    assert_eq!(result.get_number_of_nodes(), 1);
    let node = &result.get_nodes_as_vec()[0];
    let names = node.get_class_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains("paragraph"));
    assert!(names.contains("important"));
    assert!(!names.contains("nonsense"));
}
