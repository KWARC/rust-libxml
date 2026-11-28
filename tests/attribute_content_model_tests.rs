//! A node is linked only where its parent's content model has room for it (#263, #264): an attribute's value holds
//! text and entity references only, as a DOM `Attr` does and as libxml2 parses one; a DTD holds its declarations,
//! comments and processing instructions; a declaration holds no linked node.
//!
//! libxml2 links any node under an attribute, and frees the attribute's value nodes when it replaces the value
//! (`set_property`) or the attribute (`add_child` and the sibling adders with an attribute of the same name). An
//! element there could declare a namespace, which a `Namespace`, or another element's namespace, then pointed at after
//! the free. libxml2 frees no child of an attribute or element declaration (`xmlFreeAttribute`, `xmlFreeElement`), and
//! writes a DTD's children into the internal subset. Each such link is refused with `Err` now, and leaves both nodes
//! as they were.
mod common;

use common::quarantine::setup;
use libxml::parser::Parser;
use libxml::tree::{Document, Namespace, Node, NodeType};

fn new_doc_with_root() -> (Document, Node) {
  setup();
  let mut doc = Document::new().unwrap();
  let root = Node::new("root", None, &doc).unwrap();
  doc.set_root_element(&root);
  (doc, root)
}

/// #263's scenario: an element under an attribute declared a namespace, and `set_property` freed both.
#[test]
fn an_attribute_takes_no_element_child() {
  let (doc, mut root) = new_doc_with_root();
  root.set_property("a", "v").unwrap();
  let mut attr = root.get_property_node("a").unwrap();
  let mut child = Node::new("child", None, &doc).unwrap();
  assert!(attr.add_child(&mut child).is_err());
  assert!(child.is_unlinked());
  assert!(child.get_parent().is_none());
  let ns = Namespace::new("p", "urn:p", &mut child).unwrap();
  root.set_property("a", "new-value").unwrap();
  assert!(!child.is_null());
  assert_eq!(ns.get_prefix(), "p");
  assert_eq!(ns.get_href(), "urn:p");
  assert_eq!(doc.node_to_string(&root), r#"<root a="new-value"/>"#);
  // The child is still free to go elsewhere.
  root.add_child(&mut child).unwrap();
  assert_eq!(
    doc.node_to_string(&root),
    r#"<root a="new-value"><child xmlns:p="urn:p"/></root>"#
  );
}

/// #264's scenario: the attribute holding the element was replaced by one of the same name, which freed it.
#[test]
fn an_attribute_replaced_by_add_child_held_no_element() {
  let (doc, mut root) = new_doc_with_root();
  root.set_property("a", "v1").unwrap();
  let mut attr1 = root.get_property_node("a").unwrap();
  let mut child = Node::new("child", None, &doc).unwrap();
  assert!(attr1.add_child(&mut child).is_err());
  let ns = Namespace::new("p", "urn:p", &mut child).unwrap();
  let mut other = Node::new("other", None, &doc).unwrap();
  root.add_child(&mut other).unwrap();
  other.set_property("a", "v2").unwrap();
  let mut attr2 = other.get_property_node("a").unwrap();
  root.add_child(&mut attr2).unwrap();
  assert_eq!(root.get_property("a").as_deref(), Some("v2"));
  assert!(attr1.is_null());
  assert!(!child.is_null());
  assert_eq!(ns.get_prefix(), "p");
  assert_eq!(ns.get_href(), "urn:p");
}

/// Next to an attribute's value node, as under the attribute: text and entity references only.
#[test]
fn a_value_node_takes_only_text_siblings() {
  let (mut doc, mut root) = new_doc_with_root();
  root.set_property("a", "v").unwrap();
  let attr = root.get_property_node("a").unwrap();
  let mut value = attr.get_first_child().unwrap();
  assert_eq!(value.get_type(), Some(NodeType::TextNode));
  let makers: [fn(&Document) -> Node; 2] = [
    |doc| Node::new("e", None, doc).unwrap(),
    |doc| Node::new_comment("c", doc).unwrap(),
  ];
  for make in makers {
    let mut sibling = make(&doc);
    assert!(value.add_next_sibling(&mut sibling).is_err());
    assert!(value.add_prev_sibling(&mut sibling).is_err());
    assert!(sibling.is_unlinked());
  }
  let mut pi = doc.create_processing_instruction("pi", "x").unwrap();
  let mut attr = attr;
  assert!(attr.add_child(&mut pi).is_err());
  let mut text = Node::new_text("w", &doc).unwrap();
  value.add_next_sibling(&mut text).unwrap();
  assert_eq!(root.get_property("a").as_deref(), Some("vw"));
  let mut text = Node::new_text("x", &doc).unwrap();
  attr.add_child(&mut text).unwrap();
  assert_eq!(root.get_property("a").as_deref(), Some("vwx"));
}

/// A parsed attribute value with an entity reference keeps working: its reference is a value node, and moves.
#[test]
fn a_parsed_attribute_value_with_an_entity_reference_still_links() {
  setup();
  let doc = Parser::default()
    .parse_string(r#"<!DOCTYPE r [<!ENTITY e "x">]><r a="1&e;2"/>"#)
    .unwrap();
  let root = doc.get_root_element().unwrap();
  let mut attr = root.get_property_node("a").unwrap();
  let types: Vec<_> = attr.get_child_nodes().iter().map(Node::get_type).collect();
  assert_eq!(
    types,
    [
      Some(NodeType::TextNode),
      Some(NodeType::EntityRefNode),
      Some(NodeType::TextNode)
    ]
  );
  assert_eq!(root.get_property("a").as_deref(), Some("1x2"));
  let mut reference = attr.get_child_nodes()[1].clone();
  attr.add_child(&mut reference).unwrap();
  assert_eq!(root.get_property("a").as_deref(), Some("12x"));
  let mut first = attr.get_first_child().unwrap();
  first.add_prev_sibling(&mut reference).unwrap();
  assert_eq!(root.get_property("a").as_deref(), Some("x12"));
  let mut text = Node::new_text("3", &doc).unwrap();
  reference.add_next_sibling(&mut text).unwrap();
  assert_eq!(root.get_property("a").as_deref(), Some("x312"));
  assert_eq!(doc.node_to_string(&root), r#"<r a="&e;312"/>"#);
}

const DTD_XML: &str =
  r#"<!DOCTYPE r [<!ELEMENT r ANY><!ATTLIST r a CDATA "x"><!ENTITY e "y">]><r>&e;</r>"#;

fn declarations(doc: &Document) -> (Node, Vec<Node>) {
  let dtd = doc.get_root_element().unwrap().get_prev_sibling().unwrap();
  assert_eq!(dtd.get_type(), Some(NodeType::DTDNode));
  let declarations = dtd.get_child_nodes();
  assert_eq!(
    declarations.iter().map(Node::get_type).collect::<Vec<_>>(),
    [
      Some(NodeType::ElementDecl),
      Some(NodeType::AttributeDecl),
      Some(NodeType::EntityDecl)
    ]
  );
  (dtd, declarations)
}

/// libxml2 frees no child of an attribute or element declaration, and an entity declaration's children are its
/// content, which its references show: a declaration takes no child, and no element next to it, in the DTD.
#[test]
fn a_declaration_takes_no_child_and_a_dtd_no_element() {
  setup();
  let doc = Parser::default().parse_string(DTD_XML).unwrap();
  let (mut dtd, declarations) = declarations(&doc);
  for mut declaration in declarations {
    let mut element = Node::new("e", None, &doc).unwrap();
    assert!(declaration.add_child(&mut element).is_err());
    assert!(declaration.add_next_sibling(&mut element).is_err());
    assert!(declaration.add_prev_sibling(&mut element).is_err());
    let mut comment = Node::new_comment("c", &doc).unwrap();
    assert!(declaration.add_child(&mut comment).is_err());
    assert!(element.is_unlinked());
  }
  let mut element = Node::new("e", None, &doc).unwrap();
  assert!(dtd.add_child(&mut element).is_err());
  let mut text = Node::new_text("t", &doc).unwrap();
  assert!(dtd.add_child(&mut text).is_err());
  assert_eq!(
    doc.to_string(),
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE r [\n<!ELEMENT r ANY>\n<!ATTLIST r a CDATA \"x\">\n<!ENTITY e \"y\">\n]>\n<r>&e;</r>\n"
  );
}

/// A DTD holds comments and processing instructions, as libxml2 parses them in the internal subset, and the
/// document holds the DTD's siblings.
#[test]
fn a_dtd_takes_comments_and_its_siblings_are_the_documents() {
  setup();
  let mut doc = Parser::default().parse_string(DTD_XML).unwrap();
  let (mut dtd, mut declarations) = declarations(&doc);
  let mut comment = Node::new_comment("in", &doc).unwrap();
  dtd.add_child(&mut comment).unwrap();
  let mut pi = doc.create_processing_instruction("pi", "x").unwrap();
  declarations[0].add_next_sibling(&mut pi).unwrap();
  let mut before = Node::new_comment("before", &doc).unwrap();
  dtd.add_prev_sibling(&mut before).unwrap();
  assert_eq!(
    doc.to_string(),
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!--before-->\n<!DOCTYPE r [\n<!ELEMENT r ANY>\n<?pi x?><!ATTLIST r a CDATA \"x\">\n<!ENTITY e \"y\">\n<!--in-->]>\n<r>&e;</r>\n"
  );
}
