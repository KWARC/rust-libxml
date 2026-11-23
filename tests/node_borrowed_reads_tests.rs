//! `Node`'s borrowed reads (`with_name`, `with_content`, `with_namespace_prefix`, `with_namespace_href`,
//! `for_each_property(_ns)`, `for_each_attribute(_ns)`) read what `RoNode`'s read (#82), a neutralized `Node` reads as
//! empty, and `for_each_property` stops once `f` frees the node.
mod common;

use common::quarantine::{is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::{Document, Node};

const XML: &str = r#"<!DOCTYPE r [<!ENTITY e "EE">]>
<r xmlns="urn:default" xmlns:p="urn:p" a="1" empty="" ent="x&e;y" p:q="nsval" p:a="2" xml:id="i1">
  <p:c p:z="9">text<!--note--><![CDATA[cdata]]><?pi data?></p:c><plain xmlns="" k="v"/>
</r>"#;

fn parse() -> Document {
  setup();
  Parser::default().parse_string(XML).unwrap()
}

type Property = (String, Option<String>, String);

fn properties_of_node(node: &Node) -> (Vec<Property>, Vec<Property>, Vec<Property>) {
  let mut plain = Vec::new();
  node.for_each_property(|name, value| plain.push((name.into(), None, value.into())));
  let mut with_ns = Vec::new();
  node.for_each_property_ns(|name, ns, value| {
    with_ns.push((name.into(), ns.map(Into::into), value.into()))
  });
  let mut aliases = Vec::new();
  node.for_each_attribute(|name, value| aliases.push((name.into(), None, value.into())));
  node.for_each_attribute_ns(|name, ns, value| {
    aliases.push((name.into(), ns.map(Into::into), value.into()))
  });
  (plain, with_ns, aliases)
}

/// On every kind of node: an element in a default namespace, a prefixed one, one in no namespace, an attribute, and
/// text, comment, CDATA and PI nodes.
#[test]
fn node_reads_what_ronode_reads() {
  let doc = parse();
  let root = doc.get_root_element().unwrap();
  let prefixed = root.get_first_element_child().unwrap();
  let plain = prefixed.get_next_element_sibling().unwrap();
  let attr = prefixed.get_attribute_node("z").unwrap();
  let mut nodes = vec![root.clone(), prefixed.clone(), plain, attr];
  nodes.extend(prefixed.get_child_nodes());
  assert_eq!(nodes.len(), 8);
  for node in &nodes {
    let ro = node.as_readonly();
    assert_eq!(node.with_name(str::to_string), ro.with_name(str::to_string));
    assert_eq!(node.with_name(str::to_string), node.get_name());
    assert_eq!(
      node.with_content(str::to_string),
      ro.with_content(str::to_string)
    );
    assert_eq!(node.with_content(str::to_string), node.get_content());
    assert_eq!(
      node.with_namespace_prefix(|p| p.map(str::to_string)),
      ro.with_namespace_prefix(|p| p.map(str::to_string))
    );
    assert_eq!(
      node.with_namespace_href(|h| h.map(str::to_string)),
      ro.with_namespace_href(|h| h.map(str::to_string))
    );
    let (plain, with_ns, aliases) = properties_of_node(node);
    let mut ro_plain = Vec::new();
    ro.for_each_property(|name, value| ro_plain.push((name.into(), None, value.into())));
    let mut ro_with_ns = Vec::new();
    ro.for_each_property_ns(|name, ns, value| {
      ro_with_ns.push((name.into(), ns.map(Into::into), value.into()))
    });
    assert_eq!(plain, ro_plain);
    assert_eq!(with_ns, ro_with_ns);
    assert_eq!(aliases, [plain, with_ns].concat());
  }
  let (_, with_ns, _) = properties_of_node(&root);
  let names: Vec<(&str, Option<&str>, &str)> = with_ns
    .iter()
    .map(|(n, ns, v)| (n.as_str(), ns.as_deref(), v.as_str()))
    .collect();
  assert_eq!(
    names,
    [
      ("a", None, "1"),
      ("empty", None, ""),
      ("ent", None, "xEEy"),
      ("q", Some("urn:p"), "nsval"),
      ("a", Some("urn:p"), "2"),
      ("id", Some("http://www.w3.org/XML/1998/namespace"), "i1"),
    ]
  );
  assert_eq!(root.with_namespace_prefix(|p| p.map(str::len)), Some(0));
  assert!(prefixed.with_namespace_prefix(|p| p == Some("p")));
}

/// A `Node` whose node was freed (through another wrapper) reads as a node without name, content, namespace or
/// properties, as its `get_*` methods read it.
#[test]
fn a_neutralized_node_reads_as_empty() {
  let doc = parse();
  let root = doc.get_root_element().unwrap();
  let prefixed = root.get_first_element_child().unwrap();
  prefixed.clone().free_subtree();
  assert!(prefixed.is_null());
  assert_eq!(prefixed.with_name(str::to_string), "");
  assert_eq!(prefixed.with_content(str::to_string), "");
  assert_eq!(
    prefixed.with_namespace_prefix(|p| p.map(str::to_string)),
    None
  );
  assert_eq!(
    prefixed.with_namespace_href(|h| h.map(str::to_string)),
    None
  );
  let (plain, with_ns, aliases) = properties_of_node(&prefixed);
  assert!(plain.is_empty() && with_ns.is_empty() && aliases.is_empty());
}

/// `f` may change the properties: the walk goes on from the property after the one `f` removed, and reads the values
/// as they are when it reaches them.
#[test]
fn for_each_property_survives_changing_and_removing_properties() {
  let doc = parse();
  let mut root = doc.get_root_element().unwrap();
  let mut handle = root.clone();
  let mut seen = Vec::new();
  root.for_each_property(|name, value| {
    seen.push(format!("{name}={value}"));
    if name == "a" && value == "1" {
      handle.remove_property("a").unwrap();
      handle.set_property("ent", "changed").unwrap();
    }
  });
  assert_eq!(
    seen,
    ["a=1", "empty=", "ent=changed", "q=nsval", "a=2", "id=i1"]
  );
  assert!(root.get_property_no_ns("a").is_none());
  root.set_property("empty", "x").unwrap();
}

/// `f` frees the element (through a clone of the wrapper): the walk stops instead of reading the freed element again
/// (valgrind reports that read; the quarantine heap zeroes the element, which reads as a node without properties).
#[test]
fn for_each_property_stops_once_f_frees_the_node() {
  let doc = parse();
  let root = doc.get_root_element().unwrap();
  let mut prefixed = root.get_first_element_child().unwrap();
  prefixed.set_attribute("second", "2").unwrap();
  let prefixed_ptr = prefixed.node_ptr();
  let mut handle = Some(prefixed.clone());
  let mut calls = 0;
  prefixed.for_each_property(|_, _| {
    calls += 1;
    if let Some(node) = handle.take() {
      node.free_subtree();
    }
  });
  assert!(is_freed(prefixed_ptr));
  assert!(prefixed.is_null());
  assert_eq!(calls, 1);
}
