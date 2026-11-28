//! A DTD's default value is not an element's attribute: `remove_property` and its siblings leave one alone (#262).
//!
//! `xmlHasProp` and `xmlHasNsProp` also find an attribute the DTD declares with a default, and return its declaration
//! (an `xmlAttribute`, whose parent is the DTD). `xmlRemoveProp` took it for an attribute and walked the DTD's
//! attribute table as the element's attribute list, reading past its end (valgrind on libxml2 2.15.2: "Invalid read of
//! size 8 at xmlRemoveProp", 16 bytes after a 32-byte block). They now leave it alone and return `Ok`, as libxml2's
//! own `xmlUnsetProp` finds no DTD default to remove.
use libxml::parser::Parser;
use libxml::tree::NodeType;

const DTD_DEFAULT: &str =
  r#"<!DOCTYPE r [<!ATTLIST r a CDATA "dflt" p:b CDATA "pdflt">]><r xmlns:p="urn:p"/>"#;

#[test]
fn removing_a_dtd_default_is_a_no_op() {
  let doc = Parser::default().parse_string(DTD_DEFAULT).unwrap();
  let mut root = doc.get_root_element().unwrap();
  assert!(root.has_property("a"));
  assert_eq!(
    root.get_property_node("a").unwrap().get_type(),
    Some(NodeType::AttributeDecl)
  );
  assert!(root.remove_property("a").is_ok());
  assert!(root.remove_attribute("a").is_ok());
  assert!(root.remove_property_no_ns("a").is_ok());
  assert!(root.has_property("a"));
  assert_eq!(root.get_property("a").as_deref(), Some("dflt"));

  assert!(root.has_property_ns("b", "urn:p"));
  assert!(root.remove_property_ns("b", "urn:p").is_ok());
  assert!(root.has_property_ns("b", "urn:p"));
  assert_eq!(doc.node_to_string(&root), r#"<r xmlns:p="urn:p"/>"#);
}

#[test]
fn removing_an_attribute_over_a_dtd_default_uncovers_the_default() {
  let doc = Parser::default()
    .parse_string(r#"<!DOCTYPE r [<!ATTLIST r a CDATA "dflt">]><r a="set"/>"#)
    .unwrap();
  let mut root = doc.get_root_element().unwrap();
  let attr = root.get_property_node("a").unwrap();
  assert_eq!(attr.get_type(), Some(NodeType::AttributeNode));
  assert!(root.remove_property("a").is_ok());
  assert!(attr.is_null());
  // The element's attribute is gone; the DTD's default is what libxml2 finds now, and it stays.
  assert_eq!(root.get_property("a").as_deref(), Some("dflt"));
  assert!(root.remove_property("a").is_ok());
  assert_eq!(root.get_property("a").as_deref(), Some("dflt"));
  assert_eq!(doc.node_to_string(&root), "<r/>");
}

/// `get_property_node` and its siblings, of `Node` and of `RoNode`, hand out a DTD default as libxml2 finds it: its
/// declaration, of type `AttributeDecl`. Every safe method leaves the declaration as it was, or changes it as libxml2
/// changes one; none frees it, links it, or reads past its struct (run under valgrind).
#[test]
fn a_dtd_defaults_declaration_is_safe_to_use() {
  use libxml::tree::c14n::CanonicalizationOptions;
  use libxml::tree::{Namespace, Node};
  let mut doc = Parser::default().parse_string(DTD_DEFAULT).unwrap();
  let root = doc.get_root_element().unwrap();
  let ro = root.as_readonly();
  let ro_decl = ro.get_property_node("a").unwrap();
  assert_eq!(ro_decl.get_type(), Some(NodeType::AttributeDecl));
  assert_eq!(
    ro.get_property_node_ns("b", "urn:p").map(|n| n.get_type()),
    Some(Some(NodeType::AttributeDecl))
  );
  assert_eq!(ro_decl.get_name(), "a");
  assert_eq!(ro_decl.get_content(), "");
  assert!(ro_decl.get_properties().is_empty());
  assert!(ro_decl.get_namespace().is_none());
  assert!(ro_decl.get_namespace_declarations().is_empty());
  assert!(ro_decl.get_child_nodes().is_empty());
  assert_eq!(
    ro_decl.get_parent().map(|p| p.get_type()),
    Some(Some(NodeType::DTDNode))
  );
  let _ = ro_decl.lookup_namespace_uri("p");
  let _ = ro_decl.findnodes("..", &doc);

  let mut decl = root.get_property_node_no_ns("a").unwrap();
  assert_eq!(decl.get_type(), Some(NodeType::AttributeDecl));
  assert_eq!(decl.get_name(), "a");
  assert!(decl.get_properties().is_empty());
  assert!(decl.get_namespace().is_none());
  assert!(decl.get_namespaces(&doc).is_empty());
  assert!(decl.get_namespace_declarations().is_empty());
  assert!(decl.get_first_child().is_none());
  assert_eq!(decl.lookup_namespace_uri("p").as_deref(), None);
  assert!(decl.lookup_namespace_prefix("urn:p").is_none());
  assert!(decl.get_class_names().is_empty());
  let _ = decl.findnodes("..");
  let _ = decl.canonicalize(CanonicalizationOptions::default());
  // The mutators: each a no-op, or refused.
  decl.set_name("q").unwrap();
  decl.set_content("c").unwrap();
  decl.set_text_content("c").unwrap();
  decl.append_text("c").unwrap();
  decl.set_property("x", "1").unwrap();
  decl.remove_property("x").unwrap();
  decl.remove_property("a").unwrap();
  decl.recursively_remove_namespaces().unwrap();
  assert!(Namespace::new("n", "urn:n", &mut decl).is_err());
  let ns = root.get_namespace_declarations().pop().unwrap();
  let _ = decl.set_namespace(&ns);
  let mut element = Node::new("e", None, &doc).unwrap();
  assert!(decl.add_child(&mut element).is_err());
  assert!(decl.add_next_sibling(&mut element).is_err());
  assert!(decl.new_child(None, "e").is_err());
  assert!(decl.add_text_child(None, "e", "t").is_err());
  let mut pi = doc.create_processing_instruction("pi", "x").unwrap();
  decl.add_prev_sibling(&mut pi).unwrap();
  decl.unlink_node();
  assert_eq!(
    decl.get_parent().map(|p| p.get_type()),
    Some(Some(NodeType::DTDNode))
  );
  assert!(
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decl.set_rust_owned())).is_err()
  );
  assert!(
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| doc.set_root_element(&decl))).is_err()
  );
  let copy = root.get_property_node("a").unwrap();
  copy.free_subtree();
  assert_eq!(root.get_property("a").as_deref(), Some("dflt"));
  assert_eq!(decl.get_name(), "a");
  assert!(doc.to_string().contains("<!ATTLIST r a CDATA \"dflt\""));
}
