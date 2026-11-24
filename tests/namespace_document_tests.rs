//! A `Namespace` keeps its document alive, as an XPath `Context` does: it points into the document, which would
//! otherwise free it under the handle. A namespace of one document is refused on a node of another, which would point
//! into the first after it drops. libxml2's heap is quarantined (see `common::quarantine`), so a read of a freed
//! document is caught by `is_freed`.
mod common;

use std::panic::{RefUnwindSafe, UnwindSafe};

use common::quarantine::{is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::{Document, Namespace, Node};

fn parse(xml: &str) -> Document {
  setup();
  Parser::default().parse_string(xml).unwrap()
}

/// The auto traits a `Namespace` had before it held its document.
#[test]
fn a_namespace_keeps_its_auto_traits() {
  fn auto_traits<T: UnwindSafe + RefUnwindSafe + Unpin>() {}
  auto_traits::<Namespace>();
}

/// Every way a `Node` hands out a namespace, and `Namespace::new`: the document outlives its last `Document` and `Node`
/// handles while a namespace is held, and is freed with the last namespace.
#[test]
fn a_namespace_keeps_its_document_alive() {
  let doc = parse(r#"<p:r xmlns:p="urn:p" xmlns:q="urn:q" q:a="1"/>"#);
  let doc_ptr = doc.doc_ptr();
  let mut root = doc.get_root_element().unwrap();
  let held: Vec<(&str, Namespace)> = vec![
    ("get_namespace", root.get_namespace().unwrap()),
    (
      "get_namespace_declarations",
      root.get_namespace_declarations().remove(1),
    ),
    ("get_namespaces", root.get_namespaces(&doc).remove(0)),
    (
      "get_properties_ns",
      root
        .get_properties_ns()
        .into_keys()
        .find_map(|(_, ns)| ns)
        .unwrap(),
    ),
    (
      "get_attributes_ns",
      root
        .get_attributes_ns()
        .into_keys()
        .find_map(|(_, ns)| ns)
        .unwrap(),
    ),
    (
      "RoNode::get_namespaces",
      root.as_readonly().get_namespaces(&doc).remove(0),
    ),
    ("new", Namespace::new("n", "urn:n", &mut root).unwrap()),
  ];
  drop(root);
  drop(doc);
  for (how, ns) in &held {
    assert!(!is_freed(doc_ptr), "{how}: the document was freed");
    assert!(!is_freed(ns.ns_ptr()), "{how}: the namespace was freed");
    assert!(ns.get_href().starts_with("urn:"), "{how}");
  }
  drop(held);
  assert!(
    is_freed(doc_ptr),
    "the document outlived its last namespace"
  );
}

/// Each method that sets a namespace refuses one of another document, and leaves the node as it was; one of the node's
/// own document is set as before.
#[test]
fn a_namespace_of_another_document_is_refused() {
  let doc1 = parse(r#"<r xmlns:p="urn:p"/>"#);
  let p = doc1
    .get_root_element()
    .unwrap()
    .get_namespace_declarations()
    .remove(0);
  let doc2 = parse("<s/>");
  let mut s = doc2.get_root_element().unwrap();
  assert!(s.set_namespace(&p).is_err());
  assert!(s.set_property_ns("a", "1", &p).is_err());
  assert!(s.set_attribute_ns("a", "1", &p).is_err());
  assert!(s.new_child(Some(p.clone()), "c").is_err());
  assert!(s.add_text_child(Some(p.clone()), "c", "t").is_err());
  assert!(Node::new("c", Some(p.clone()), &doc2).is_err());
  // A namespace from an `RoNode` too, although it does not keep its document alive.
  let ro_p = doc1
    .get_root_readonly()
    .unwrap()
    .get_namespace_declarations()
    .remove(0);
  assert!(s.set_namespace(&ro_p).is_err());
  assert!(s.set_property_ns("a", "1", &ro_p).is_err());
  assert!(s.get_namespace().is_none());
  assert!(s.get_properties().is_empty());
  assert!(s.get_child_nodes().is_empty());
  assert_eq!(
    doc2.to_string(),
    Parser::default().parse_string("<s/>").unwrap().to_string()
  );

  let mut q = Namespace::new("q", "urn:q", &mut s).unwrap();
  s.set_namespace(&q).unwrap();
  s.set_property_ns("a", "1", &q).unwrap();
  let mut c = s.new_child(Some(q.clone()), "c").unwrap();
  s.add_text_child(Some(q.clone()), "t", "x").unwrap();
  let made = Node::new("m", Some(q.clone()), &doc2).unwrap();
  assert_eq!(made.get_namespace().unwrap().get_href(), "urn:q");
  c.set_namespace(&q).unwrap();
  assert_eq!(s.get_property_ns("a", "urn:q").as_deref(), Some("1"));
  #[allow(deprecated)]
  q.free();
}

/// A namespace declared on a node that is then freed: the document keeps the declaration (#238), and the namespace
/// keeps the document.
#[test]
fn a_namespace_of_a_freed_node_keeps_its_document_alive() {
  let doc = parse("<r><c/></r>");
  let doc_ptr = doc.doc_ptr();
  let mut c = doc
    .get_root_element()
    .unwrap()
    .get_first_element_child()
    .unwrap();
  let ns = Namespace::new("p", "urn:p", &mut c).unwrap();
  c.free_subtree();
  drop(doc);
  assert!(!is_freed(doc_ptr));
  assert!(!is_freed(ns.ns_ptr()));
  assert_eq!(ns.get_href(), "urn:p");
  drop(ns);
  assert!(is_freed(doc_ptr));
}

/// `RoNode::get_namespaces` with another document than the node's: its namespaces are the node's document's, so they
/// do not keep the other alive (nor the node's, which an `RoNode` does not know), and a node of the other refuses them.
#[test]
fn ronode_get_namespaces_with_another_document_keeps_neither() {
  let doc1 = parse(r#"<r xmlns:p="urn:p"/>"#);
  let doc2 = parse("<s/>");
  let doc2_ptr = doc2.doc_ptr();
  let root = doc1.get_root_element().unwrap();
  let p = root.as_readonly().get_namespaces(&doc2).remove(0);
  assert_eq!(p.get_href(), "urn:p");
  let mut s = doc2.get_root_element().unwrap();
  assert!(s.set_namespace(&p).is_err());
  root.clone().set_namespace(&p).unwrap();
  drop((s, doc2));
  assert!(
    is_freed(doc2_ptr),
    "the namespace kept the other document alive"
  );
}

/// A node without a document (made in a `Document` without a libxml2 document, or neutralized) refuses any namespace:
/// a namespace of a document is that document's, and the node would point into it after it drops.
#[test]
fn a_node_without_a_document_refuses_a_namespace() {
  let doc = parse(r#"<p:r xmlns:p="urn:p"/>"#);
  let p = doc.get_root_element().unwrap().get_namespace().unwrap();
  let no_document = unsafe { Document::from_raw_ptr(std::ptr::null_mut()) };
  assert!(Node::new("x", Some(p.clone()), &no_document).is_err());
  let mut neutralized = Node::null();
  assert!(neutralized.set_namespace(&p).is_err());
  assert!(neutralized.set_property_ns("a", "1", &p).is_err());
  assert!(neutralized.new_child(Some(p.clone()), "c").is_err());
}
