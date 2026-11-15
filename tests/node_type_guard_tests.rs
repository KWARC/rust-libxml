//! The namespace reads and the property walks read only the node types whose struct holds those fields.
//!
//! libxml2's `xmlDoc`, `xmlDtd` and compact text nodes share `xmlNode`'s prefix but put other data where an element
//! keeps its `ns` and `properties`: a document's `compression`/`standalone` ints sit in `ns`, a DTD's `notations` table
//! too, and a text node parsed with `compact` holds its characters in `properties`. Read as pointers, those were a
//! SIGSEGV (or garbage); every namespace read now answers "no namespace" off an element or attribute, and every
//! property walk answers "no properties" off an element.
//!
//! XInclude's start and end nodes are the exception: libxml2 turns the `xi:include` element into the start node and
//! copies it into the end node (`xinclude.c`), so both keep an element's `ns` and `properties`, and both read them.

use libxml::parser::{Parser, ParserOptions};
use libxml::tree::{Document, Node, NodeType};

const DTD_XML: &str = r#"<!DOCTYPE r [<!NOTATION gif PUBLIC "image/gif"><!ELEMENT r ANY><!ATTLIST r a CDATA "x">]><r xmlns="urn:r"/>"#;

fn dtd_node(doc: &Document) -> Node {
  let dtd = doc.get_root_element().unwrap().get_prev_sibling().unwrap();
  assert_eq!(dtd.get_type(), Some(NodeType::DTDNode));
  dtd
}

fn assert_no_namespace(node: &Node) {
  assert!(node.get_namespace().is_none());
  let ro = node.as_readonly();
  assert!(ro.get_namespace().is_none());
  assert_eq!(ro.with_namespace_prefix(|p| p.map(str::to_string)), None);
  assert_eq!(ro.with_namespace_href(|h| h.map(str::to_string)), None);
}

#[test]
fn document_node_has_no_namespace() {
  for xml in [
    "<r xmlns='urn:r'/>",
    "<?xml version='1.0' standalone='yes'?><r xmlns='urn:r'/>",
  ] {
    let doc = Parser::default().parse_string(xml).unwrap();
    let node = doc.as_node();
    assert_eq!(node.get_type(), Some(NodeType::DocumentNode));
    assert_no_namespace(&node);
    // The root element still reads its namespace.
    let root = doc.get_root_element().unwrap();
    assert_eq!(root.get_namespace().unwrap().get_href(), "urn:r");
    assert!(
      root
        .as_readonly()
        .with_namespace_href(|h| h == Some("urn:r"))
    );
  }
  assert_no_namespace(&Document::new().unwrap().as_node());
}

#[test]
fn dtd_node_has_no_namespace() {
  let doc = Parser::default().parse_string(DTD_XML).unwrap();
  assert_no_namespace(&dtd_node(&doc));
}

/// A text node short enough for `compact` keeps its characters in the `properties` slot (libxml2's
/// `xmlSAX2TextNode`); the walks read no properties there, and the element's own are unchanged.
#[test]
fn compact_text_node_has_no_properties() {
  let options = ParserOptions {
    compact: true,
    ..ParserOptions::default()
  };
  let doc = Parser::default()
    .parse_string_with_options("<r a='1' xmlns:p='urn:p' p:b='2'>hi</r>", options)
    .unwrap();
  let root = doc.get_root_element().unwrap();
  let text = root.get_first_child().unwrap();
  assert_eq!(text.get_type(), Some(NodeType::TextNode));
  assert_eq!(text.get_content(), "hi");

  assert!(text.get_properties().is_empty());
  assert!(text.get_properties_ns().is_empty());
  assert!(text.get_attributes().is_empty());
  assert!(text.get_attributes_ns().is_empty());
  let ro = text.as_readonly();
  assert!(ro.get_properties().is_empty());
  assert!(ro.get_properties_ns().is_empty());
  assert!(ro.get_attributes().is_empty());
  assert!(ro.get_attributes_ns().is_empty());
  let mut calls = 0;
  ro.for_each_property(|_, _| calls += 1);
  ro.for_each_property_ns(|_, _, _| calls += 1);
  ro.for_each_attribute(|_, _| calls += 1);
  ro.for_each_attribute_ns(|_, _, _| calls += 1);
  assert_eq!(calls, 0);

  assert_eq!(root.get_properties().len(), 2);
  assert_eq!(root.as_readonly().get_properties().len(), 2);
  let mut seen = Vec::new();
  root
    .as_readonly()
    .for_each_property_ns(|n, ns, v| seen.push(format!("{n}|{ns:?}|{v}")));
  assert_eq!(seen, ["a|None|1", "b|Some(\"urn:p\")|2"]);
}

/// The document and DTD nodes, whose `properties`-slot data is not a property list, walk no properties either.
#[test]
fn document_and_dtd_nodes_have_no_properties() {
  let doc = Parser::default().parse_string(DTD_XML).unwrap();
  for node in [doc.as_node(), dtd_node(&doc)] {
    assert!(node.get_properties().is_empty());
    assert!(node.get_properties_ns().is_empty());
    assert!(node.as_readonly().get_properties().is_empty());
    let mut calls = 0;
    node.as_readonly().for_each_property(|_, _| calls += 1);
    assert_eq!(calls, 0);
  }
}

/// XInclude processing turns the `xi:include` element into an `XML_XINCLUDE_START` node and adds an
/// `XML_XINCLUDE_END` copy of it after the included content; both keep the element's namespace, and the start node its
/// properties, which the namespace reads and property walks still read.
#[test]
fn xinclude_nodes_keep_their_namespace_and_properties() {
  const XI: &str = "http://www.w3.org/2001/XInclude";
  // A URI, not a path: on Windows the manifest directory has backslashes and a drive letter, which libxml2 resolves
  // only as a `file:///D:/...` URI.
  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/resources/file02.xml").replace('\\', "/");
  let href = if cfg!(windows) {
    format!("file:///{path}")
  } else {
    path
  };
  let xml = format!(
    r#"<r xmlns:xi="{XI}" xmlns:p="urn:p"><xi:include href="{href}" parse="text" p:note="kept"/></r>"#
  );
  let doc = Parser::default().parse_string(xml).unwrap();
  let root = doc.get_root_element().unwrap();
  let include = root.get_first_child().unwrap();
  let props = |node: &Node| node.get_properties();
  let element_props = props(&include);
  assert_eq!(element_props.len(), 3);
  // SAFETY: the document is alive and no wrapper is borrowed across the call.
  assert_eq!(
    unsafe { libxml::bindings::xmlXIncludeProcess(doc.doc_ptr()) },
    1
  );

  let start = root.get_first_child().unwrap();
  let end = root.get_last_child().unwrap();
  assert_eq!(start, include);
  assert_eq!(start.get_type(), Some(NodeType::XIncludeStart));
  assert_eq!(end.get_type(), Some(NodeType::XIncludeEnd));
  assert_eq!(
    start.get_next_sibling().unwrap().get_type(),
    Some(NodeType::TextNode)
  );
  for node in [&start, &end] {
    assert_eq!(node.get_name(), "include");
    assert_eq!(node.get_namespace().unwrap().get_href(), XI);
    let ro = node.as_readonly();
    assert_eq!(ro.get_namespace().unwrap().get_href(), XI);
    assert_eq!(
      ro.with_namespace_prefix(|p| p.map(str::to_string))
        .as_deref(),
      Some("xi")
    );
    assert!(ro.with_namespace_href(|h| h == Some(XI)));
  }
  assert_eq!(props(&start), element_props);
  let ro = start.as_readonly();
  assert_eq!(ro.get_properties().len(), 3);
  assert_eq!(ro.get_attributes_ns().len(), 3);
  let mut seen = Vec::new();
  ro.for_each_property_ns(|n, ns, v| seen.push(format!("{n}|{ns:?}|{v}")));
  assert_eq!(
    seen,
    [
      format!("href|None|{href}"),
      "parse|None|text".to_string(),
      "note|Some(\"urn:p\")|kept".to_string()
    ]
  );
  // The end node is a copy without the properties.
  assert!(props(&end).is_empty());
}
