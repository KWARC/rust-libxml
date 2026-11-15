//! The borrowed reads (`RoNode::with_name`, `with_namespace_prefix`, `with_namespace_href`, `for_each_property`,
//! `for_each_property_ns`) read what the copying accessors read, and `Node::as_readonly` / `node_from_readonly` pass
//! between the two node types without losing the document's wrapper.

use std::collections::HashMap;

use libxml::parser::Parser;
use libxml::tree::{Document, Node, NodeType};
use libxml::xpath::Context;

const XML: &str = r#"<!DOCTYPE r [<!ENTITY e "EE"><!ATTLIST r d CDATA "dflt">]>
<r xmlns="urn:default" xmlns:p="urn:p" a="1" empty="" amp="x&amp;y" ent="x&e;y" p:q="nsval" p:a="2" xml:id="i1">
  <p:c p:z="9">text</p:c><plain xmlns="" k="v"/>
</r>"#;

/// Every shape of value reads as `get_properties` reads it: plain, empty, a predefined entity, an unsubstituted entity
/// reference (the copying fallback), namespaced ones, `xml:id`; a DTD default is not a property in either. Two
/// properties sharing a local name (`a`, `p:a`) both reach the visitor, in document order; `Node::get_properties` keeps
/// the last, `RoNode::get_properties` the first.
#[test]
fn for_each_property_reads_what_get_properties_reads() {
  let doc = Parser::default().parse_string(XML).unwrap();
  let root = doc.get_root_element().unwrap();
  let mut seen = Vec::new();
  root
    .as_readonly()
    .for_each_property(|name, value| seen.push((name.to_string(), value.to_string())));
  let names: Vec<&str> = seen.iter().map(|(n, _)| n.as_str()).collect();
  assert_eq!(names, ["a", "empty", "amp", "ent", "q", "a", "id"]);
  let last_wins: HashMap<String, String> = seen.iter().cloned().collect();
  assert_eq!(last_wins, root.get_properties());
  assert_eq!(last_wins["ent"], "xEEy");
  assert_eq!(last_wins["amp"], "x&y");
  assert_eq!(last_wins["a"], "2");
  // `RoNode::get_properties` looks each name up again (`xmlGetProp`), which finds the first `a`.
  assert_eq!(root.as_readonly().get_properties()["a"], "1");

  let mut with_ns = Vec::new();
  root.as_readonly().for_each_property_ns(|name, ns, value| {
    with_ns.push((name.to_string(), ns.map(str::to_string), value.to_string()))
  });
  let expected: Vec<(String, Option<String>, String)> = [
    ("a", None, "1"),
    ("empty", None, ""),
    ("amp", None, "x&y"),
    ("ent", None, "xEEy"),
    ("q", Some("urn:p"), "nsval"),
    ("a", Some("urn:p"), "2"),
    ("id", Some("http://www.w3.org/XML/1998/namespace"), "i1"),
  ]
  .into_iter()
  .map(|(n, ns, v)| (n.to_string(), ns.map(str::to_string), v.to_string()))
  .collect();
  assert_eq!(with_ns, expected);
  let by_ns: HashMap<(String, Option<String>), String> =
    with_ns.into_iter().map(|(n, ns, v)| ((n, ns), v)).collect();
  let copied: HashMap<(String, Option<String>), String> = root
    .get_properties_ns()
    .into_iter()
    .map(|((n, ns), v)| ((n, ns.map(|ns| ns.get_href())), v))
    .collect();
  assert_eq!(by_ns, copied);

  // A node without properties, and a text node, call nothing.
  let text = root
    .get_first_element_child()
    .unwrap()
    .get_first_child()
    .unwrap();
  let mut calls = 0;
  text.as_readonly().for_each_property(|_, _| calls += 1);
  assert_eq!(calls, 0);
}

/// The borrowed name and namespace reads give what `get_name` and `get_namespace()`'s `get_prefix` / `get_href` give,
/// for an element in a default namespace, a prefixed one, one in no namespace, an attribute node and a text node.
#[test]
fn with_name_and_namespace_read_what_the_copies_read() {
  let doc = Parser::default().parse_string(XML).unwrap();
  let root = doc.get_root_element().unwrap();
  let prefixed = root.get_first_element_child().unwrap();
  let plain = prefixed.get_next_element_sibling().unwrap();
  let text = prefixed.get_first_child().unwrap();
  let attr = prefixed.get_attribute_node("z").unwrap();
  for node in [&root, &prefixed, &plain, &text, &attr] {
    let ro = node.as_readonly();
    assert_eq!(ro.with_name(str::to_string), node.get_name());
    assert_eq!(
      ro.with_namespace_prefix(|p| p.map(str::to_string)),
      node.get_namespace().map(|ns| ns.get_prefix())
    );
    assert_eq!(
      ro.with_namespace_href(|h| h.map(str::to_string)),
      node.get_namespace().map(|ns| ns.get_href())
    );
  }
  assert_eq!(
    root
      .as_readonly()
      .with_namespace_prefix(|p| p.map(str::len)),
    Some(0)
  );
  assert!(
    prefixed
      .as_readonly()
      .with_namespace_prefix(|p| p == Some("p"))
  );
  assert!(plain.as_readonly().with_namespace_href(|h| h.is_none()));
  assert_eq!(text.as_readonly().with_name(str::to_string), "text");
}

/// `node_from_readonly` returns the wrapper the document already holds: a node created and not yet linked keeps its
/// linkage through the round trip, from a `Node` and from the `Document` alike.
#[test]
fn node_from_readonly_returns_the_documents_wrapper() {
  let mut doc = Document::new().unwrap();
  let mut root = Node::new("root", None, &doc).unwrap();
  doc.set_root_element(&root);
  let created = Node::new("created", None, &doc).unwrap();
  assert!(created.is_unlinked());
  let again = root.node_from_readonly(created.as_readonly());
  assert_eq!(again, created);
  assert!(
    again.is_unlinked(),
    "the cached wrapper, not a new linked one"
  );
  assert!(doc.node_from_readonly(created.as_readonly()).is_unlinked());

  let mut child = created.clone();
  root.add_child(&mut child).unwrap();
  assert!(!root.node_from_readonly(created.as_readonly()).is_unlinked());
  assert_eq!(doc.node_from_readonly(root.as_readonly()), root);
}

/// A read-only walk over a tree changed between its steps: a node created and linked mid-walk is reached and wraps
/// as the node that created it; a node unlinked mid-walk (its next sibling read first) leaves the walk on the tree.
#[test]
fn readonly_walk_sees_nodes_created_and_unlinked_mid_walk() {
  let doc = Parser::default()
    .parse_string("<r><a/><b/><c/></r>")
    .unwrap();
  let mut root = doc.get_root_element().unwrap();
  let mut visited = Vec::new();
  let mut added = None;
  let mut cur = root.as_readonly().get_first_child();
  while let Some(ro) = cur {
    let next = ro.get_next_sibling();
    let name = ro.with_name(str::to_string);
    visited.push(name.clone());
    if name == "a" {
      let mut new = Node::new("d", None, &doc).unwrap();
      root.add_child(&mut new).unwrap();
      added = Some(new);
    }
    if name == "b" {
      root.node_from_readonly(ro).unlink_node();
    }
    cur = next;
  }
  assert_eq!(visited, ["a", "b", "c", "d"]);
  let added = added.unwrap();
  let last = root.as_readonly().get_last_child().unwrap();
  assert_eq!(root.node_from_readonly(last), added);
  let names: Vec<String> = root
    .get_child_nodes()
    .iter()
    .filter(|n| n.get_type() == Some(NodeType::ElementNode))
    .map(Node::get_name)
    .collect();
  assert_eq!(names, ["a", "c", "d"]);
  assert_eq!(doc.node_to_string(&root), "<r><a/><c/><d/></r>");
}

/// `with_content` gives what `get_content` gives, for the node types whose content libxml2 holds in the node (text,
/// CDATA, comment, processing instruction) and for those it gathers (an element, an attribute, an entity reference).
#[test]
fn with_content_reads_what_get_content_reads() {
  let doc = Parser::default()
    .parse_string(
      r#"<!DOCTYPE r [<!ENTITY e "EE">]><r a="x&e;y">t&amp;u<![CDATA[<c>]]><!-- note --><?pi data?>&e;<s>deep</s></r>"#,
    )
    .unwrap();
  let root = doc.get_root_element().unwrap();
  let attr = root.get_attribute_node("a").unwrap();
  let mut nodes = vec![root.clone(), attr];
  nodes.extend(root.get_child_nodes());
  let types: Vec<Option<NodeType>> = nodes.iter().map(Node::get_type).collect();
  assert!(types.contains(&Some(NodeType::CDataSectionNode)));
  assert!(types.contains(&Some(NodeType::CommentNode)));
  assert!(types.contains(&Some(NodeType::PiNode)));
  assert!(types.contains(&Some(NodeType::EntityRefNode)));
  for node in &nodes {
    assert_eq!(
      node.as_readonly().with_content(str::to_string),
      node.get_content(),
      "{:?}",
      node.get_type()
    );
  }
  assert_eq!(
    root.as_readonly().with_content(str::to_string),
    "t&u<c>EEdeep"
  );
}

/// `for_each_attribute` and `for_each_attribute_ns` are `for_each_property` and `for_each_property_ns`.
#[test]
fn for_each_attribute_aliases_for_each_property() {
  let doc = Parser::default().parse_string(XML).unwrap();
  let root = doc.get_root_element().unwrap().as_readonly();
  let (mut props, mut attrs) = (Vec::new(), Vec::new());
  root.for_each_property(|n, v| props.push(format!("{n}={v}")));
  root.for_each_attribute(|n, v| attrs.push(format!("{n}={v}")));
  assert_eq!(props.len(), 7);
  assert_eq!(attrs, props);
  let (mut props, mut attrs) = (Vec::new(), Vec::new());
  root.for_each_property_ns(|n, ns, v| props.push(format!("{n}|{ns:?}|{v}")));
  root.for_each_attribute_ns(|n, ns, v| attrs.push(format!("{n}|{ns:?}|{v}")));
  assert_eq!(attrs, props);
}

/// A string that is not UTF-8 reads lossily, as the copying accessors read it (U+FFFD for the bad byte). libxml2's
/// parser never puts such a string in the tree (it rejects the input or, recovering, re-decodes it as Latin-1); the
/// tree API does, unchecked, so the test writes it through the raw bindings.
#[test]
fn borrowed_reads_decode_invalid_utf8_lossily() {
  use libxml::bindings::{
    xmlNewNs, xmlNodeSetContent, xmlNodeSetName, xmlSetNs, xmlSetNsProp, xmlSetProp,
  };
  let doc = Parser::default().parse_string("<r>t</r>").unwrap();
  let root = doc.get_root_element().unwrap();
  let text = root.get_first_child().unwrap();
  unsafe {
    xmlNodeSetName(root.node_ptr(), c"n\xffm".as_ptr().cast());
    xmlNodeSetContent(text.node_ptr(), c"c\xfed".as_ptr().cast());
    let ns = xmlNewNs(
      root.node_ptr(),
      c"urn:\xff".as_ptr().cast(),
      c"p\xff".as_ptr().cast(),
    );
    xmlSetNs(root.node_ptr(), ns);
    xmlSetProp(
      root.node_ptr(),
      c"k\xff".as_ptr().cast(),
      c"v\xff".as_ptr().cast(),
    );
    xmlSetNsProp(
      root.node_ptr(),
      ns,
      c"q".as_ptr().cast(),
      c"w".as_ptr().cast(),
    );
  }
  let ro = root.as_readonly();
  assert_eq!(ro.with_name(str::to_string), "n\u{FFFD}m");
  assert_eq!(ro.with_name(str::to_string), root.get_name());
  assert_eq!(
    text.as_readonly().with_content(str::to_string),
    "c\u{FFFD}d"
  );
  assert_eq!(
    text.as_readonly().with_content(str::to_string),
    text.get_content()
  );
  assert_eq!(
    ro.with_namespace_prefix(|p| p.map(str::to_string))
      .as_deref(),
    Some("p\u{FFFD}")
  );
  assert_eq!(
    ro.with_namespace_href(|h| h.map(str::to_string)).as_deref(),
    Some("urn:\u{FFFD}")
  );
  let mut seen = Vec::new();
  ro.for_each_property_ns(|n, ns, v| seen.push(format!("{n}|{ns:?}|{v}")));
  assert_eq!(
    seen,
    ["k\u{FFFD}|None|v\u{FFFD}", "q|Some(\"urn:\u{FFFD}\")|w"]
  );
  let copied: HashMap<String, String> = seen
    .iter()
    .map(|s| {
      let parts: Vec<&str> = s.split('|').collect();
      (parts[0].to_string(), parts[2].to_string())
    })
    .collect();
  assert_eq!(copied, root.get_properties());
}

/// `node_from_readonly` gives `Node::null()` for a node of another document, in release builds too, and caches nothing:
/// a wrapper cached in the wrong document would outlive the node's own document.
#[test]
fn node_from_readonly_rejects_a_node_of_another_document() {
  let doc = Parser::default().parse_string("<r><a/></r>").unwrap();
  let root = doc.get_root_element().unwrap();
  let other = Parser::default().parse_string("<o><x/></o>").unwrap();
  let foreign = other.get_root_readonly().unwrap();
  for wrapped in [
    doc.node_from_readonly(foreign),
    doc.node_from_readonly(foreign.get_first_child().unwrap()),
    root.node_from_readonly(foreign),
  ] {
    assert!(wrapped.is_null());
  }
  drop(other);
  // The document's own nodes still wrap, and the foreign ones left nothing behind to drop.
  let a = doc.get_root_readonly().unwrap().get_first_child().unwrap();
  assert_eq!(doc.node_from_readonly(a).get_name(), "a");
  doc.trim_node_cache();
}

/// A namespace node from the XPath namespace axis is an `xmlNs`, not an `xmlNode`: `get_readonly_nodes_as_vec` skips
/// it, so `node_from_readonly` (which would give `Node::null()` for one) never wraps the shorter struct, whose sibling
/// read overflowed it.
#[test]
fn node_from_readonly_rejects_a_namespace_node() {
  let doc = Parser::default()
    .parse_string(r#"<r xmlns:p="urn:p"><a/></r>"#)
    .unwrap();
  let root = doc.get_root_element().unwrap();
  let context = Context::new(&doc).unwrap();
  let namespaces = context.evaluate("//namespace::*").unwrap();
  assert!(namespaces.get_number_of_nodes() > 0);
  assert!(namespaces.get_readonly_nodes_as_vec().is_empty());
  drop(namespaces);
  doc.trim_node_cache();
  assert_eq!(doc.node_to_string(&root), r#"<r xmlns:p="urn:p"><a/></r>"#);
}

/// A node that is in the document but in no tree (unlinked through libxml2, never wrapped) wraps as unlinked, as
/// the crate's other accessors wrap such a node; the document node itself, which has no parent either, as linked.
#[test]
fn node_from_readonly_wraps_an_unlinked_node_as_unlinked() {
  let doc = Parser::default().parse_string("<r><a/><b/></r>").unwrap();
  let a = doc.get_root_readonly().unwrap().get_first_child().unwrap();
  unsafe { libxml::bindings::xmlUnlinkNode(a.node_ptr()) };
  let wrapped = doc.node_from_readonly(a);
  assert!(wrapped.is_unlinked());
  assert_eq!(wrapped.get_name(), "a");
  // (owned by its wrapper, so the document's drop frees it)
  wrapped.set_rust_owned();
  let document_node = doc.get_root_readonly().unwrap().get_parent().unwrap();
  assert_eq!(document_node.get_type(), Some(NodeType::DocumentNode));
  assert!(!doc.node_from_readonly(document_node).is_unlinked());
  let b = doc.get_root_readonly().unwrap().get_first_child().unwrap();
  assert!(!doc.node_from_readonly(b).is_unlinked());
}

/// A closure may remove the property it is shown: the walk has read the next one first (under AddressSanitizer, the
/// old order read the freed attribute).
#[test]
fn for_each_property_survives_removing_the_current_property() {
  let doc = Parser::default()
    .parse_string(r#"<r xmlns:p="urn:p" a="1" p:b="2" c="3"/>"#)
    .unwrap();
  let root = doc.get_root_element().unwrap();
  let mut seen = Vec::new();
  root.as_readonly().for_each_property_ns(|name, ns, value| {
    seen.push(format!("{name}|{}|{value}", ns.unwrap_or("")));
    let mut element = root.clone();
    match ns {
      Some(ns) => element.remove_property_ns(name, ns),
      None => element.remove_property(name),
    }
    .unwrap();
  });
  assert_eq!(seen, ["a||1", "b|urn:p|2", "c||3"]);
  assert!(root.get_properties_ns().is_empty());
  let mut names = Vec::new();
  let doc = Parser::default()
    .parse_string(r#"<r a="1" b="2"/>"#)
    .unwrap();
  let root = doc.get_root_element().unwrap();
  root.as_readonly().for_each_property(|name, _| {
    names.push(name.to_string());
    root.clone().remove_property(name).unwrap();
  });
  assert_eq!(names, ["a", "b"]);
  assert!(root.get_properties().is_empty());
}
