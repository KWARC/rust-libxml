//! A few random tests.
//! Knowing how much I neglect this aspect of software development,
//! there probably won't be a significant coverage.

extern crate libxml;

use libxml::tree::{Document, Node, Namespace};
use libxml::xpath::{XmlXPathContext};
use libxml::parser::{Parser, ParseFormat};

#[test]
/// Build a hello world XML doc
fn hello_builder() {
    let doc_result = Document::new();
    assert!(doc_result.is_ok());
    let mut doc = doc_result.unwrap();
    
    let hello_element_result = Node::new("hello", None, Some(&doc));
    assert!(hello_element_result.is_ok());
    let mut hello_element = hello_element_result.unwrap();

    let mock_ns_result = Namespace::new(&hello_element, "http://example.com/ns/mock", "mock");
    assert!(mock_ns_result.is_ok());
    // let mock_ns = mock_ns_result.unwrap();

    doc.set_root_element(&mut hello_element);

    hello_element.set_content("world!");

    doc.save_file("tests/results/helloworld.xml").unwrap();
}


#[test]
/// Duplicate an xml file
fn duplicate_file() {
    let parser = Parser::default();
    
    let doc_parse = parser.parse_file("tests/resources/file01.xml");
    assert!(doc_parse.is_ok());
    
    let doc = doc_parse.unwrap();
    doc.save_file("tests/results/copy.xml").unwrap();
}

#[test]
/// Can load an HTML file
fn can_load_html_file() {
  let parser = Parser {format : ParseFormat::HTML };
  
  let doc_parse = parser.parse_file("tests/resources/example.html");
  assert!(doc_parse.is_ok());

  let doc = doc_parse.unwrap();
  assert_eq!(doc.get_root_element().unwrap().get_name(),"html");
}

#[test]
/// Root node and first child of root node are different
/// (There is a tiny chance this might fail for a correct program)
fn child_of_root_has_different_hash() {
  let parser = Parser::default();
  let doc = parser.parse_file("tests/resources/file01.xml").unwrap();
  let root = doc.get_root_element().unwrap();
  assert!(!root.is_text_node());
  if let Some(child) = root.get_first_child() {
    assert!(root != child);
  } else {
    assert!(false);   //test failed - child doesn't exist
  }
}


#[test]
/// Test the evaluation of an xpath expression yields the correct number of nodes
fn test_xpath_result_number_correct() {
  let parser = Parser::default();
  let doc = parser.parse_file("tests/resources/file01.xml").unwrap();
  let context = XmlXPathContext::new(&doc).unwrap();

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
  let parser = Parser { format : ParseFormat::HTML};
  let doc = parser.parse_file("tests/resources/file02.xml").unwrap();
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
