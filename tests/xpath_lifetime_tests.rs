//! An XPath `Context` or `Object` keeps its document alive (#61): libxml2 evaluates against the document, and an
//! object's node set points into it.
mod common;

use common::quarantine::{double_freed, is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::{Document, Node};
use libxml::xpath::Context;

fn parse(xml: &str) -> Document {
  setup();
  Parser::default().parse_string(xml).unwrap()
}

/// #61's pattern: the document goes out of scope where the context is made.
fn context_of(xml: &str) -> Context {
  let doc = parse(xml);
  Context::new(&doc).unwrap()
}

#[test]
fn a_context_keeps_its_document_alive() {
  let mut ctx = context_of("<r><a>x</a><a>y</a></r>");
  assert_eq!(ctx.findvalues("//a", None).unwrap(), ["x", "y"]);
  let a = ctx.findnodes("//a", None).unwrap();
  assert_eq!(a.len(), 2);
  assert_eq!(a[1].get_content(), "y");
}

#[test]
fn a_context_lets_go_of_its_document_when_it_drops() {
  let doc = parse("<r><a>x</a><a>y</a></r>");
  let doc_ptr = doc.doc_ptr();
  let ctx = Context::new(&doc).unwrap();
  let clone = ctx.clone();
  drop(doc);
  assert!(!is_freed(doc_ptr), "the context keeps the document");
  assert_eq!(ctx.evaluate("//a").unwrap().get_number_of_nodes(), 2);
  drop(ctx);
  assert!(!is_freed(doc_ptr), "its clone keeps the document");
  drop(clone);
  assert!(is_freed(doc_ptr) && !double_freed(doc_ptr));
}

#[test]
fn an_object_keeps_its_document_alive() {
  let doc = parse("<r><a>x</a><a>y</a></r>");
  let doc_ptr = doc.doc_ptr();
  let object = Context::new(&doc).unwrap().evaluate("//a").unwrap();
  drop(doc);
  assert!(!is_freed(doc_ptr), "the object keeps the document");
  assert_eq!(object.get_nodes_as_str(), ["x", "y"]);
  assert_eq!(object.to_string(), "x");
  let a = object.get_nodes_as_vec();
  assert_eq!(a[0].get_content(), "x");
  assert_eq!(object.get_readonly_nodes_as_vec()[1].get_content(), "y");
  drop(object);
  assert!(is_freed(doc_ptr) && !double_freed(doc_ptr));
  // As after any document's drop, its wrappers read as `Node::null()`.
  assert!(a[0].is_null());
}

#[test]
fn from_node_without_a_document_is_an_error() {
  setup();
  assert!(Context::from_node(&Node::null()).is_err());
  let doc = parse("<r/>");
  let r = doc.get_root_element().unwrap();
  drop(doc);
  assert!(Context::from_node(&r).is_err());
}
