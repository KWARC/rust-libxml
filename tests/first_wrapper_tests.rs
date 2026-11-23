//! The methods that free attributes or their value nodes look for wrappers to neutralize only once such a wrapper may
//! exist, which is noted process-wide and never reset. Each step below is the first of its kind in this process, so it
//! must be noted for its wrapper to be neutralized. The steps share one test, in order, because a step that ran before
//! in this process would make a later one pass anyway; the other test binaries wrap these nodes freely.
mod common;

use common::quarantine::{is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::Node;

fn assert_neutralized(what: &str, node: &Node) {
  assert!(
    !is_freed(node.node_ptr()),
    "the {what} wrapper points at a node libxml2 freed"
  );
  assert!(node.is_null(), "the {what} wrapper is not neutralized");
}

#[test]
fn the_first_wrapper_of_an_attribute_or_value_node_is_noted() {
  setup();
  let doc = Parser::default().parse_string("<r a='1' b=''/>").unwrap();
  let mut root = doc.get_root_element().unwrap();

  // An attribute is wrapped, and no value node yet: `remove_property` frees it.
  let a = root.get_property_node("a").unwrap();
  root.remove_property("a").unwrap();
  assert_neutralized("removed attribute's", &a);

  // A wrapped node is moved under an attribute, and no value node was wrapped: `set_property` frees it.
  let mut b = root.get_property_node("b").unwrap();
  // The parser gives `b` an empty text node, which `add_child` would merge into; an empty content is no node.
  b.set_content("").unwrap();
  assert!(
    b.get_first_child().is_none(),
    "the attribute has a value node"
  );
  let mut text = Node::new_text("x", &doc).unwrap();
  b.add_child(&mut text).unwrap();
  assert_eq!(root.get_property("b").as_deref(), Some("x"));
  root.set_property("b", "y").unwrap();
  assert_neutralized("moved value node's", &text);
  assert_eq!(root.get_property("b").as_deref(), Some("y"));
}
