//! `set_name` renames an element, an attribute, a processing instruction or an entity reference, and leaves any other
//! node alone, with `Ok`, as libxml2 2.13 and later do, with every libxml2 version (#265).
//!
//! Before 2.13, `xmlNodeSetName` also renamed a DTD, a document and a DTD's declarations. A parsed document interns
//! names in its dictionary, which an element declaration's free (`xmlFreeElement`) then passed to `xmlFree`: an
//! invalid free when the document dropped. libxml2's heap is quarantined (see `common::quarantine`), so that free is
//! recorded, not performed.
mod common;

use common::quarantine::setup;
use libxml::parser::Parser;
use libxml::tree::{Node, NodeType};

const XML: &str = r#"<!DOCTYPE r [<!ELEMENT r ANY><!ATTLIST r a CDATA "x"><!ENTITY e "y">]><r a="1"><?pi x?>&e;<!--c-->t</r>"#;

#[test]
fn set_name_leaves_a_dtd_and_its_declarations_alone() {
  setup();
  let doc = Parser::default().parse_string(XML).unwrap();
  let root = doc.get_root_element().unwrap();
  let dtd = root.get_prev_sibling().unwrap();
  assert_eq!(dtd.get_type(), Some(NodeType::DTDNode));
  let mut unnamed: Vec<Node> = dtd.get_child_nodes();
  assert_eq!(
    unnamed.iter().map(Node::get_type).collect::<Vec<_>>(),
    [
      Some(NodeType::ElementDecl),
      Some(NodeType::AttributeDecl),
      Some(NodeType::EntityDecl)
    ]
  );
  unnamed.push(dtd);
  unnamed.push(doc.as_node());
  let children = root.get_child_nodes();
  assert_eq!(children[2].get_type(), Some(NodeType::CommentNode));
  assert_eq!(children[3].get_type(), Some(NodeType::TextNode));
  unnamed.extend_from_slice(&children[2..]);
  for mut node in unnamed {
    let name = node.get_name();
    assert!(node.set_name("q").is_ok());
    assert_eq!(node.get_name(), name, "{:?} was renamed", node.get_type());
  }
  drop(root);
  assert!(doc.to_string().contains("<!ELEMENT r ANY>"));
}

#[test]
fn set_name_renames_an_element_an_attribute_and_a_processing_instruction() {
  setup();
  let doc = Parser::default().parse_string(XML).unwrap();
  let mut root = doc.get_root_element().unwrap();
  let mut attr = root.get_property_node("a").unwrap();
  let mut pi = root.get_first_child().unwrap();
  assert_eq!(pi.get_type(), Some(NodeType::PiNode));
  root.set_name("s").unwrap();
  attr.set_name("b").unwrap();
  pi.set_name("qi").unwrap();
  assert_eq!(
    doc.node_to_string(&root),
    r#"<s b="1"><?qi x?>&e;<!--c-->t</s>"#
  );
}
