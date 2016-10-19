//! A few random tests.
//! Knowing how much I neglect this aspect of software development,
//! there probably won't be a significant coverage.

extern crate libxml;

use std::fs::File;
use std::io::Read;

use libxml::tree::{Document, Node, Namespace};
use libxml::xpath::Context;
use libxml::parser::Parser;

#[test]
/// Build a hello world XML doc
fn hello_builder() {
  let doc_result = Document::new();
  assert!(doc_result.is_ok());
  let mut doc = doc_result.unwrap();

  let hello_element_result = Node::new("hello", None, &doc);
  assert!(hello_element_result.is_ok());
  let mut hello_element = hello_element_result.unwrap();

  let mock_ns_result = Namespace::new(&hello_element, "http://example.com/ns/mock", "mock");
  assert!(mock_ns_result.is_ok());
  // let mock_ns = mock_ns_result.unwrap();

  doc.set_root_element(&mut hello_element);

  hello_element.set_content("world!");

  let added = hello_element.new_child(None, "child");
  assert!(added.is_ok());
  let new_child = added.unwrap();

  new_child.set_content("set content");

  let node_string = doc.node_to_string(&hello_element);
  assert!(node_string.len() > 1);

  let doc_string = doc.to_string();
  assert!(doc_string.len() > 1);
  assert!(doc.save_file("tests/results/helloworld.xml").is_ok());
}

#[test]
fn create_pi() {
  let doc_result = Document::new();
  assert!(doc_result.is_ok());
  let mut doc = doc_result.unwrap();
  // Add a PI
  let node_ok = doc.create_processing_instruction("piname", "picontent");
  assert!(node_ok.is_ok());
  let doc_string = doc.to_string();
  println!("{:?}", doc_string);
  assert!(doc_string.len() > 1);
}


#[test]
/// Duplicate an xml file
fn duplicate_file() {
  let parser = Parser::default();
  {
    let doc_result = parser.parse_file("tests/resources/file01.xml");
    assert!(doc_result.is_ok());

    let doc = doc_result.unwrap();
    doc.save_file("tests/results/copy.xml").unwrap();
  }
}

#[test]
// Can parse an xml string in memory
fn can_parse_xml_string() {
  let mut file = File::open("tests/resources/file01.xml").unwrap();
  let mut xml_string = String::new();
  file.read_to_string(&mut xml_string).unwrap();
  let parser = Parser::default();
  let doc = parser.parse_string(&xml_string).unwrap();
  assert_eq!(doc.get_root_element().unwrap().get_name(), "root");
}

#[test]
/// Can load an HTML file
fn can_load_html_file() {
  let parser = Parser::default_html();
  {
    let doc_result = parser.parse_file("tests/resources/example.html");
    assert!(doc_result.is_ok());

    let doc = doc_result.unwrap();
    let root_result = doc.get_root_element();
    assert!(root_result.is_ok());
    let root = root_result.unwrap();
    assert_eq!(root.get_name(), "html");
  }
}

#[test]
/// Root node and first child of root node are different
/// (There is a tiny chance this might fail for a correct program)
fn child_of_root_has_different_hash() {
  let parser = Parser::default();
  {
    let doc_result = parser.parse_file("tests/resources/file01.xml");
    assert!(doc_result.is_ok());
    let doc = doc_result.unwrap();
    let root = doc.get_root_element().unwrap();
    assert!(!root.is_text_node());
    if let Some(child) = root.get_first_child() {
      assert!(root != child);
    } else {
      assert!(false);   //test failed - child doesn't exist
    }
  }
}

#[test]
/// Siblings basic unit tests
fn sibling_unit_tests() {
  let mut doc = Document::new().unwrap();
  let hello_element_result = Node::new("hello", None, &doc);
  assert!(hello_element_result.is_ok());
  let mut hello_element = hello_element_result.unwrap();
  doc.set_root_element(&mut hello_element);

  let new_sibling = Node::new("sibling", None, &doc).unwrap();
  assert!(hello_element.add_prev_sibling(new_sibling).is_some());
}

#[test]
/// Test the evaluation of an xpath expression yields the correct number of nodes
fn test_xpath_result_number_correct() {
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let context = Context::new(&doc).unwrap();

  let result1 = context.evaluate("//child").unwrap();
  assert_eq!(result1.get_number_of_nodes(), 2);
  assert_eq!(result1.get_nodes_as_vec().len(), 2);

  let result2 = context.evaluate("//nonexistent").unwrap();
  assert_eq!(result2.get_number_of_nodes(), 0);
  assert_eq!(result2.get_nodes_as_vec().len(), 0);
}


#[test]
/// Test that an xpath expression finds the correct node and
/// that the class names are interpreted correctly.
fn test_class_names() {
  let parser = Parser::default_html();
  let doc_result = parser.parse_file("tests/resources/file02.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let context = Context::new(&doc).unwrap();

  let p_result = context.evaluate("/html/body/p");
  assert!(p_result.is_ok());
  let p = p_result.unwrap();
  assert_eq!(p.get_number_of_nodes(), 1);

  let node = &p.get_nodes_as_vec()[0];
  let names = node.get_class_names();
  assert_eq!(names.len(), 2);
  assert!(names.contains("paragraph"));
  assert!(names.contains("important"));
  assert!(!names.contains("nonsense"));
}

#[test]
/// Test that an xpath string() function processed correctly
fn test_xpath_string_function() {
  let parser = Parser::default_html();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let context = Context::new(&doc).unwrap();

  let p_result = context.evaluate("string(//root//child[1]/@attribute)");
  assert!(p_result.is_ok());
  let p = p_result.unwrap();
  // Not a node really
  assert_eq!(p.get_number_of_nodes(), 0);
  let content = p.to_string();
  assert_eq!(content, "value");
}

#[test]
/// Test well-formedness of a Rust string
/// IMPORTANT: Currenlty NOT THREAD-SAFE, use in single-threaded apps only!
fn test_well_formed_html() {
  let parser = Parser::default_html();

  let trivial_well_formed = parser.is_well_formed_html("<!DOCTYPE html>\n<html><head></head><body></body></html>");
  assert!(trivial_well_formed);

  let trivial_ill_formed = parser.is_well_formed_html("garbage");
  assert!(!trivial_ill_formed);

  let should_ill_formed = parser.is_well_formed_html("<broken <markup>> </boom>");
  assert!(!should_ill_formed);

  let should_well_formed = parser.is_well_formed_html("<!DOCTYPE html>\n<html><head><title>Test</title></head><body>\n<h1>Tiny</h1><math><mn>2</mn></math></body></html>");
  assert!(should_well_formed);
}

#[test]
/// Can mock a node object (useful for defaults that will be overridden)
fn test_can_mock_node() {
  let node_mock = Node::mock();
  assert!(!node_mock.is_text_node());
}

#[test]
/// Can make a mock node hashable
fn test_can_hash_mock_node() {
  let node_mock = Node::mock();
  assert!(node_mock.to_hashable() > 0);
}
