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
  let root = doc.get_root_element();

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

  first_a.set_attribute("attribute", "newa");

  assert_eq!(
    first_a.get_attribute("attribute"),
    Some(String::from("newa"))
  );

  // This currently PASSES - which is WRONG - we need
  //1) compile error when first_b is assigned
  // and
  //2) it should never be possible that an immutable libxml variable changes value
  assert_eq!(
    first_b.get_attribute("attribute"),
    Some(String::from("newa"))
  );

}
