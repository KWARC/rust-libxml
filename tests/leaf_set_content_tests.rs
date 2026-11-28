//! `set_content` on a processing instruction, a comment or a CDATA section replaces its text and leaves its children
//! alone, as libxml2 2.13 and later do, with every libxml2 version (#261).
//!
//! libxml2 links a child under these nodes (`add_child`). Before 2.13, `xmlNodeSetContent` also freed their children,
//! whose wrappers then pointed at freed nodes, and a `Namespace` declared on one at a freed `xmlNs`; since, it sets
//! the text only. libxml2's heap is quarantined (see `common::quarantine`), so a node freed on an older libxml2 is
//! caught by `is_freed`, without valgrind.
mod common;

use common::quarantine::{is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::{Document, Namespace, Node, NodeType};

/// `leaf` takes an element child that declares a namespace; then `set` replaces `leaf`'s text, which the child
/// survives.
fn assert_children_survive(doc: &Document, mut leaf: Node, set: fn(&mut Node, &str)) {
  let mut child = Node::new("el", None, doc).unwrap();
  leaf.add_child(&mut child).unwrap();
  let ns = Namespace::new("p", "urn:p", &mut child).unwrap();
  set(&mut leaf, "new");
  assert!(!is_freed(child.node_ptr()), "the child was freed");
  assert!(!child.is_null());
  assert_eq!(leaf.get_first_child(), Some(child.clone()));
  assert_eq!(child.get_name(), "el");
  assert_eq!(ns.get_prefix(), "p");
  assert_eq!(ns.get_href(), "urn:p");
  assert_eq!(leaf.get_content(), "new");
}

fn leaves(doc: &mut Document) -> Vec<Node> {
  let mut root = doc.get_root_element().unwrap();
  let cdata = root.get_first_child().unwrap();
  assert_eq!(cdata.get_type(), Some(NodeType::CDataSectionNode));
  let mut pi = doc.create_processing_instruction("pi", "old").unwrap();
  root.add_child(&mut pi).unwrap();
  let mut comment = Node::new_comment("old", doc).unwrap();
  root.add_child(&mut comment).unwrap();
  vec![pi, comment, cdata]
}

#[test]
fn set_content_leaves_the_children_of_a_leaf_alone() {
  setup();
  let mut doc = Parser::default()
    .parse_string("<r><![CDATA[old]]></r>")
    .unwrap();
  for leaf in leaves(&mut doc) {
    assert_children_survive(&doc, leaf, |leaf, content| {
      leaf.set_content(content).unwrap()
    });
  }
  let root = doc.get_root_element().unwrap();
  assert_eq!(
    doc.node_to_string(&root),
    "<r><![CDATA[new]]><?pi new?><!--new--></r>"
  );
}

#[test]
fn set_text_content_leaves_the_children_of_a_leaf_alone() {
  setup();
  let mut doc = Parser::default()
    .parse_string("<r><![CDATA[old]]></r>")
    .unwrap();
  for leaf in leaves(&mut doc) {
    assert_children_survive(&doc, leaf, |leaf, text| {
      leaf.set_text_content(text).unwrap()
    });
  }
}
