//! `Document::copy_node` copies a node into a document, from this document or another, linked or not (#102).
mod common;

use common::quarantine::{is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::{Document, Node, NodeType};

fn parse(xml: &str) -> Document {
  setup();
  Parser::default().parse_string(xml).unwrap()
}

const SOURCE: &str = "<r xmlns:p='urn:p'><p:a x='1' p:y='2'><b>t</b></p:a><c/>tail<!--k--></r>";

/// The copies, of siblings, are unlinked nodes of the target, and outlive the source, which is left as it was.
#[test]
fn copy_node_copies_linked_nodes_of_another_document() {
  let source = parse(SOURCE);
  let before = source.to_string();
  let root = source.get_root_element().unwrap();
  let a = root.get_first_child().unwrap();
  let c = a.get_next_sibling().unwrap();
  let tail = c.get_next_sibling().unwrap();
  let comment = root.get_last_child().unwrap();
  let mut target = Document::new().unwrap();

  let a_copy = target.copy_node(&a).unwrap();
  let mut copies = vec![];
  for node in [&c, &tail, &comment] {
    copies.push(target.copy_node(node).unwrap());
  }
  for copy in copies.iter().chain([&a_copy]) {
    assert!(copy.is_unlinked() && copy.get_parent().is_none());
  }
  for node in [&a, &c, &tail, &comment] {
    assert!(!node.is_unlinked() && node.get_parent() == Some(root.clone()));
  }
  assert_eq!(source.to_string(), before);

  let source_ptr = source.doc_ptr();
  drop((root, a, c, tail, comment, source));
  assert!(is_freed(source_ptr));

  target.set_root_element(&a_copy);
  let mut parent = a_copy.clone();
  for copy in &mut copies {
    parent.add_child(copy).unwrap();
  }
  assert_eq!(
    target.node_to_string(&a_copy),
    "<p:a xmlns:p=\"urn:p\" x=\"1\" p:y=\"2\"><b>t</b><c/>tail<!--k--></p:a>"
  );
}

#[test]
fn copy_node_copies_within_a_document() {
  let doc = parse(SOURCE);
  let root = doc.get_root_element().unwrap();
  let a = root.get_first_child().unwrap();
  let mut c = a.get_next_sibling().unwrap();
  let mut b_copy = doc.copy_node(&a.get_first_child().unwrap()).unwrap();
  c.add_child(&mut b_copy).unwrap();
  assert_eq!(
    doc.node_to_string(&root),
    "<r xmlns:p=\"urn:p\"><p:a x=\"1\" p:y=\"2\"><b>t</b></p:a><c><b>t</b></c>tail<!--k--></r>"
  );
}

/// A copy that is never linked is freed with its document once it is rust-owned, as a node from `Node::new` is.
#[test]
fn a_rust_owned_copy_is_freed() {
  let doc = parse(SOURCE);
  let copy = doc.copy_node(&doc.get_root_element().unwrap()).unwrap();
  let copy_ptr = copy.node_ptr();
  copy.set_rust_owned();
  drop(copy);
  drop(doc);
  assert!(is_freed(copy_ptr));
}

#[test]
fn copy_node_refuses_what_is_no_tree_node() {
  let source = parse("<!DOCTYPE r [<!ENTITY e 'x'>]><r a='1'/>");
  let target = Document::new().unwrap();
  let root = source.get_root_element().unwrap();
  let attribute = root.get_property_node("a").unwrap();
  let dtd = source.as_node().get_first_child().unwrap();
  assert_eq!(dtd.get_type(), Some(NodeType::DTDNode));
  let declaration = dtd.get_first_child().unwrap();
  assert_eq!(declaration.get_type(), Some(NodeType::EntityDecl));
  for node in [source.as_node(), attribute, dtd, declaration, Node::null()] {
    assert!(target.copy_node(&node).is_err(), "{:?}", node.get_type());
  }
}
