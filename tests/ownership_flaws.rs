//! Enforce Rust ownership pragmatics for the underlying libxml2 objects

extern crate libxml;

use libxml::parser::Parser;

#[test]
fn ownership_guards() {
  // Setup
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let root = doc.get_root_element().unwrap();

  let mut first_a = root.get_first_element_child().unwrap();
  let first_b = root.get_first_element_child().unwrap();

  assert_eq!(
    first_a.get_attribute("attribute"),
    Some(String::from("value"))
  );
  assert_eq!(
    first_b.get_attribute("attribute"),
    Some(String::from("value"))
  );

  // Setting an attribute will fail and return an error, as there are too many Rc references
  // to the same node (Rc strong count of 3)
  // see `Node::node_ptr_mut` for details
  assert!(first_a.set_attribute("attribute", "newa").is_err());

  assert_eq!(
    first_a.get_attribute("attribute"),
    Some(String::from("value"))
  );
  assert_eq!(
    first_b.get_attribute("attribute"),
    Some(String::from("value"))
  );
}
