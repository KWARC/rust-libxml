//! Regression tests for NULL-node FFI safety.
//!
//! A `Node::null()` placeholder wraps a NULL `xmlNodePtr`. Every field accessor
//! must degrade to the natural "absence" value instead of dereferencing address
//! 0 — e.g. `get_type()` reads the node's `type_` at **offset 8**, so a raw
//! deref on NULL is the `segfault at 8` observed in production. A crash here
//! bypasses `catch_unwind` and takes down the whole process. These tests pin
//! the null guards in `src/c_helpers.rs`; before them, every assertion below
//! was a SIGSEGV rather than a value.

use libxml::tree::Node;

#[test]
fn null_node_get_type_is_none() {
  // The exact production fault: `xmlGetNodeType(NULL)` reading offset 8.
  assert_eq!(Node::null().get_type(), None);
}

#[test]
fn null_node_is_not_element() {
  assert!(!Node::null().is_element_node());
}

#[test]
fn null_node_navigation_is_none() {
  let n = Node::null();
  assert!(n.get_parent().is_none());
  assert!(n.get_first_child().is_none());
  assert!(n.get_last_child().is_none());
  assert!(n.get_next_sibling().is_none());
  assert!(n.get_prev_sibling().is_none());
  assert!(n.get_first_element_child().is_none());
  assert!(n.get_next_element_sibling().is_none());
  assert!(n.get_prev_element_sibling().is_none());
}

#[test]
fn null_node_name_and_content_are_empty() {
  let n = Node::null();
  assert_eq!(n.get_name(), String::new());
  assert_eq!(n.get_content(), String::new());
}

// ---- borrowed reads (`RoNode::with_*`, `for_each_*`, `node_from_readonly`) on NULL -------

use libxml::parser::Parser;
use libxml::readonly::RoNode;

#[test]
fn null_ronode_borrowed_reads_are_empty() {
  let n = RoNode::null();
  assert_eq!(n.with_name(str::to_string), "");
  assert_eq!(n.with_content(str::to_string), "");
  assert_eq!(n.with_namespace_prefix(|p| p.map(str::to_string)), None);
  assert_eq!(n.with_namespace_href(|h| h.map(str::to_string)), None);
  let mut calls = 0;
  n.for_each_property(|_, _| calls += 1);
  n.for_each_property_ns(|_, _, _| calls += 1);
  n.for_each_attribute(|_, _| calls += 1);
  n.for_each_attribute_ns(|_, _, _| calls += 1);
  assert_eq!(calls, 0);
  assert!(Node::null().as_readonly().is_null());
}

/// A NULL `RoNode` wraps as `Node::null()`, not as a linked wrapper cached under the NULL key.
#[test]
fn null_ronode_wraps_as_null_node() {
  let doc = Parser::default().parse_string("<r/>").unwrap();
  let root = doc.get_root_element().unwrap();
  for wrapped in [
    doc.node_from_readonly(RoNode::null()),
    root.node_from_readonly(RoNode::null()),
  ] {
    assert!(wrapped.is_null());
    assert!(
      wrapped.is_unlinked(),
      "Node::null(), not a cached linked wrapper"
    );
    assert_eq!(wrapped.get_type(), None);
  }
}
