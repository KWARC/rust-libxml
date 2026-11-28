//! A node whose `Document` is gone (dropped, never had one, or freed under the handle) gets an error, or reads as
//! having no children, instead of a panic from unwrapping its document.
use libxml::parser::Parser;
use libxml::tree::c14n::CanonicalizationOptions;
use libxml::tree::{Document, Node};

/// `canonicalize` of a node without a document, or of a handle to a freed node, is an error. A freed node's handle used
/// to canonicalize nothing and return `Ok("")`.
#[test]
fn canonicalize_without_a_node_or_a_document_is_an_error() {
  assert!(
    Node::null()
      .canonicalize(CanonicalizationOptions::default())
      .is_err()
  );

  let mut of_dropped = {
    let doc = Parser::default().parse_string("<r><c/></r>").unwrap();
    doc.get_root_element().unwrap()
  };
  assert!(
    of_dropped
      .canonicalize(CanonicalizationOptions::default())
      .is_err()
  );

  let doc = Parser::default().parse_string("<r><c/></r>").unwrap();
  let c = doc.get_root_element().unwrap().get_first_child().unwrap();
  let mut handle = c.clone();
  c.free_subtree();
  assert!(
    handle
      .canonicalize(CanonicalizationOptions::default())
      .is_err()
  );

  let mut root = doc.get_root_element().unwrap();
  assert_eq!(
    root
      .canonicalize(CanonicalizationOptions::default())
      .unwrap(),
    "<r></r>"
  );
}

/// A node made in a `Document` without a libxml2 document is in no libxml2 document, and outlives its `Document` once
/// `dup_from` gives the `Document` one. Its children are not wrapped, and `new_child` and `add_text_child` refuse to
/// add one, leaving it as it was.
#[test]
fn a_node_whose_document_dropped_wraps_no_child() {
  let source = Parser::default().parse_string("<s/>").unwrap();
  let mut no_document = unsafe { Document::from_raw_ptr(std::ptr::null_mut()) };
  let mut orphan = Node::new("a", None, &no_document).unwrap();
  orphan.new_child(None, "b").unwrap();
  no_document.dup_from(&source).unwrap();
  drop(no_document);
  assert!(!orphan.is_null());

  assert!(orphan.get_first_child().is_none());
  assert!(orphan.get_last_child().is_none());
  assert!(orphan.get_child_nodes().is_empty());
  assert!(orphan.get_child_elements().is_empty());
  assert!(orphan.new_child(None, "c").is_err());
  assert!(orphan.add_text_child(None, "t", "x").is_err());
  let ro = orphan.as_readonly();
  assert_eq!(ro.get_child_nodes().len(), 1, "a refused child was added");
  assert_eq!(ro.get_first_child().unwrap().get_name(), "b");
}
