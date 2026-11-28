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
