//! An XPath `Object`'s node set and a `Context`'s context node never reach a node freed within their live document:
//! the `Object` drops such nodes from its set (also before it frees the set, which reads every entry), and the `Context`
//! forgets such a context node, as a new context has none. A free of any other node changes neither.
mod common;

use common::quarantine::{is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::{Document, Node};
use libxml::xpath::{Context, Expression, Object};

fn parse(xml: &str) -> Document {
  setup();
  Parser::default().parse_string(xml).unwrap()
}

const XML: &str = "<r><a id='1'>x</a><a id='2'>y</a><b>t</b><a id='3'>z</a></r>";

/// Every way an `Object` reads its nodes sees only the live ones, and none is freed.
fn assert_reads(obj: &Object, expected: &[&str]) {
  assert_eq!(obj.get_number_of_nodes(), expected.len());
  let nodes = obj.get_nodes_as_vec();
  assert!(nodes.iter().all(|node| !is_freed(node.node_ptr())));
  assert_eq!(
    nodes.iter().map(Node::get_content).collect::<Vec<_>>(),
    expected
  );
  let readonly = obj.get_readonly_nodes_as_vec();
  assert!(readonly.iter().all(|node| !is_freed(node.node_ptr())));
  assert_eq!(obj.get_nodes_as_str(), expected);
  assert_eq!(obj.to_string(), expected.first().copied().unwrap_or(""));
}

fn root(doc: &Document) -> Node {
  doc.get_root_element().unwrap()
}

#[test]
fn an_object_leaves_out_a_node_freed_by_free_subtree() {
  let doc = parse(XML);
  let obj = Context::new(&doc).unwrap().evaluate("//a").unwrap();
  let first = root(&doc).get_first_child().unwrap();
  let first_ptr = first.node_ptr();
  first.free_subtree();
  assert!(is_freed(first_ptr));
  assert_reads(&obj, &["y", "z"]);
}

/// The common loop: free every result while the `Object` is alive, then drop it (`xmlXPathFreeNodeSet` reads the type
/// of each entry; valgrind reports the read of a freed one).
#[test]
fn an_object_whose_nodes_were_all_freed_reads_empty_and_drops() {
  let doc = parse(XML);
  let obj = Context::new(&doc).unwrap().evaluate("//a").unwrap();
  for node in obj.get_nodes_as_vec() {
    node.free_subtree();
  }
  assert_reads(&obj, &[]);
  drop(obj);
}

#[test]
fn a_free_of_another_node_keeps_the_results() {
  let doc = parse(XML);
  let obj = Context::new(&doc).unwrap().evaluate("//a").unwrap();
  let b = root(&doc).findnodes("b").unwrap().pop().unwrap();
  b.free_subtree();
  assert_reads(&obj, &["x", "y", "z"]);
}

#[test]
fn an_object_leaves_out_the_children_set_content_frees() {
  let doc = parse(XML);
  let obj = Context::new(&doc).unwrap().evaluate("//a | //b").unwrap();
  root(&doc).set_content("new").unwrap();
  assert_reads(&obj, &[]);
}

#[test]
fn an_object_leaves_out_an_attribute_remove_property_frees() {
  let doc = parse(XML);
  let obj = Context::new(&doc).unwrap().evaluate("//@id").unwrap();
  let mut second = root(&doc).findnodes("a[2]").unwrap().pop().unwrap();
  second.remove_property("id").unwrap();
  assert_reads(&obj, &["1", "3"]);
}

/// `xmlAddChild` merges a text node added after a text node into it, and frees it.
#[test]
fn an_object_leaves_out_a_text_node_merged_away() {
  let doc = parse(XML);
  let obj = Context::new(&doc).unwrap().evaluate("//a/text()").unwrap();
  let mut b = root(&doc).findnodes("b").unwrap().pop().unwrap();
  let mut y = root(&doc).findnodes("a[2]/text()").unwrap().pop().unwrap();
  y.unlink_node();
  b.add_child(&mut y).unwrap();
  assert!(y.is_null(), "merged into b's text");
  assert_eq!(b.get_content(), "ty");
  assert_reads(&obj, &["x", "z"]);
}

/// A rust-owned node is freed when its last wrapper drops.
#[test]
fn an_object_leaves_out_a_rust_owned_node_freed_by_its_last_wrapper() {
  let doc = parse(XML);
  let obj = Context::new(&doc).unwrap().evaluate("//a").unwrap();
  let mut first = root(&doc).get_first_child().unwrap();
  let first_ptr = first.node_ptr();
  first.unlink_node();
  first.set_rust_owned();
  drop(first);
  assert!(is_freed(first_ptr));
  assert_reads(&obj, &["y", "z"]);
}

/// A context node stays the context's current node after an evaluation at it, as libxml2's `xmlXPathNodeEval` leaves it.
/// A namespace node is a copy the set owns, whose `next` is its element: `xmlXPathFreeNodeSet` reads that element's
/// type to tell the copy from a declaration (valgrind reports the read of a freed one). It goes with its element.
#[test]
fn an_object_leaves_out_the_namespace_nodes_of_a_freed_element() {
  let doc = parse("<r xmlns:p='urn:p'><e xmlns:q='urn:q'/><f/></r>");
  let ctx = Context::new(&doc).unwrap();
  let of_e = ctx.evaluate("//e/namespace::*").unwrap();
  let of_f = ctx.evaluate("//f/namespace::*").unwrap();
  assert_eq!(of_e.get_number_of_nodes(), 3);
  let e = root(&doc).get_first_child().unwrap();
  let e_ptr = e.node_ptr();
  e.free_subtree();
  assert!(is_freed(e_ptr));
  assert_eq!(of_e.get_number_of_nodes(), 0);
  assert!(of_e.get_nodes_as_str().is_empty());
  drop(of_e);
  let mut of_f_hrefs = of_f.get_nodes_as_str();
  of_f_hrefs.sort();
  assert_eq!(
    of_f_hrefs,
    ["http://www.w3.org/XML/1998/namespace", "urn:p"]
  );
  drop(of_f);
}

#[test]
fn a_live_context_node_stays_current() {
  let doc = parse(XML);
  let ctx = Context::new(&doc).unwrap();
  let second = root(&doc).findnodes("a[2]").unwrap().pop().unwrap();
  assert_eq!(
    ctx
      .node_evaluate("string(@id)", &second)
      .unwrap()
      .to_string(),
    "2"
  );
  assert_eq!(ctx.evaluate("string(@id)").unwrap().to_string(), "2");
  let expression = Expression::compile("string(.)").unwrap();
  let third = root(&doc).findnodes("a[3]").unwrap().pop().unwrap();
  assert_eq!(
    ctx
      .node_evaluate_compiled(&expression, &third)
      .unwrap()
      .to_string(),
    "z"
  );
  assert_eq!(ctx.evaluate_compiled(&expression).unwrap().to_string(), "z");
}

/// Each way to set the context node, then a free of that node: the context forgets it, and evaluates as a new context
/// does, at no node, with absolute paths unchanged.
#[test]
fn a_freed_context_node_is_forgotten() {
  let fresh_doc = parse(XML);
  let fresh = Context::new(&fresh_doc)
    .unwrap()
    .evaluate("count(.)")
    .unwrap()
    .to_string();
  type SetNode = fn(&mut Context, &Node);
  let setters: [(&str, SetNode); 4] = [
    ("set_context_node", |ctx, node| {
      ctx.set_context_node(node).unwrap()
    }),
    ("node_evaluate", |ctx, node| {
      ctx.node_evaluate("count(.)", node).unwrap();
    }),
    ("node_evaluate_readonly", |ctx, node| {
      ctx
        .node_evaluate_readonly("count(.)", node.as_readonly())
        .unwrap();
    }),
    ("node_evaluate_compiled", |ctx, node| {
      ctx
        .node_evaluate_compiled(&Expression::compile("count(.)").unwrap(), node)
        .unwrap();
    }),
  ];
  for (setter, set_node) in setters {
    let doc = parse(XML);
    let mut ctx = Context::new(&doc).unwrap();
    let second = root(&doc).findnodes("a[2]").unwrap().pop().unwrap();
    set_node(&mut ctx, &second);
    let second_ptr = second.node_ptr();
    // Through a fresh wrapper, so that the context's own wrapper is not the one that frees.
    root(&doc)
      .findnodes("a[2]")
      .unwrap()
      .pop()
      .unwrap()
      .free_subtree();
    assert!(is_freed(second_ptr), "{setter}");
    let at_node = ctx.evaluate("count(.)").map(|obj| obj.to_string());
    assert_eq!(at_node.as_deref(), Ok(fresh.as_str()), "{setter}");
    assert_eq!(
      ctx.evaluate("count(//a)").unwrap().to_string(),
      "2",
      "{setter}"
    );
    let compiled = ctx.evaluate_compiled(&Expression::compile("count(.)").unwrap());
    assert_eq!(
      compiled.map(|obj| obj.to_string()).ok().as_deref(),
      Some(fresh.as_str()),
      "{setter}"
    );
  }
}

/// An attribute's value node, reached as an `RoNode` (no `Node` wraps it), as the context node: `set_property` frees it
/// when it replaces the value, and the context forgets it.
#[test]
fn a_context_node_set_property_frees_is_forgotten() {
  let fresh_doc = parse("<r/>");
  let fresh = Context::new(&fresh_doc)
    .unwrap()
    .evaluate("count(.)")
    .unwrap()
    .to_string();
  let doc = parse(XML);
  let ctx = Context::new(&doc).unwrap();
  let mut first = root(&doc).get_first_child().unwrap();
  let value = first
    .as_readonly()
    .get_property_node("id")
    .unwrap()
    .get_first_child()
    .unwrap();
  assert_eq!(
    ctx
      .node_evaluate_readonly("string(.)", value)
      .unwrap()
      .to_string(),
    "1"
  );
  first.set_property("id", "one").unwrap();
  assert!(is_freed(value.node_ptr()));
  assert_eq!(ctx.evaluate("count(.)").unwrap().to_string(), fresh);
}

/// The common loop, with nothing read between the frees and the drop: the drop leaves out the freed nodes, and ends
/// the document's watch once.
#[test]
fn an_object_dropped_right_after_its_nodes_were_freed_unwatches_once() {
  let doc = parse(XML);
  let ctx = Context::new(&doc).unwrap();
  let obj = ctx.evaluate("//a").unwrap();
  for node in obj.get_nodes_as_vec() {
    node.free_subtree();
  }
  drop(obj);
  // A later set into the same document is watched and read as usual.
  let obj = ctx.evaluate("//b").unwrap();
  // With the `a`s gone, `b` is the first child.
  root(&doc).get_first_child().unwrap().free_subtree();
  assert_reads(&obj, &[]);
}
