//! A string with an interior NUL byte, which no C string holds, panicked in the methods that pass it to libxml2. Now
//! a query reads no such property or namespace, and a mutator or constructor returns `Err` and changes nothing.
//! Neither reads the string up to its NUL (`"a\0b"` is not `"a"`, which these documents have).
use libxml::parser::Parser;
use libxml::readonly::RoNode;
use libxml::tree::c14n::CanonicalizationOptions;
use libxml::tree::{Document, Namespace, Node};

const XML: &str = r#"<r xmlns:p="urn:p" a="1" p:b="2">text<!--c--></r>"#;

fn parse() -> Document {
  Parser::default().parse_string(XML).unwrap()
}

/// A string with a NUL, prefixed by `prefix`: a short one, which `with_c_str` copies to the stack, and a long one.
fn with_nul(prefix: &str) -> [String; 2] {
  [
    format!("{prefix}\0b"),
    format!("{prefix}\0{}", "b".repeat(200)),
  ]
}

#[test]
fn node_queries_read_no_such_property_or_namespace() {
  let doc = parse();
  let root = doc.get_root_element().unwrap();
  for (name, ns) in with_nul("a").iter().zip(with_nul("urn:p").iter()) {
    assert_eq!(root.get_property(name), None);
    assert_eq!(root.get_attribute(name), None);
    assert_eq!(root.get_property_no_ns(name), None);
    assert_eq!(root.get_property_ns("b", ns), None);
    assert_eq!(root.get_property_ns(name, "urn:p"), None);
    assert_eq!(root.get_attribute_ns("b", ns), None);
    assert!(root.get_property_node(name).is_none());
    assert!(root.get_property_node_no_ns(name).is_none());
    assert!(root.get_property_node_ns("b", ns).is_none());
    assert!(root.get_property_node_ns(name, "urn:p").is_none());
    assert!(!root.has_property(name));
    assert!(!root.has_attribute(name));
    assert!(!root.has_property_no_ns(name));
    assert!(!root.has_property_ns("b", ns));
    assert!(!root.has_property_ns(name, "urn:p"));
    assert!(!root.has_attribute_ns("b", ns));
    assert_eq!(root.lookup_namespace_prefix(ns), None);
    assert_eq!(root.lookup_namespace_uri(&name.replacen('a', "p", 1)), None);
  }
  assert_eq!(root.get_property("a").as_deref(), Some("1"));
  assert_eq!(root.lookup_namespace_uri("p").as_deref(), Some("urn:p"));
}

#[test]
fn ronode_queries_read_no_such_property_or_namespace() {
  let doc = parse();
  let root: RoNode = doc.get_root_readonly().unwrap();
  for (name, ns) in with_nul("a").iter().zip(with_nul("urn:p").iter()) {
    assert_eq!(root.get_property(name), None);
    assert_eq!(root.get_attribute(name), None);
    assert_eq!(root.get_property_no_ns(name), None);
    assert_eq!(root.get_property_ns("b", ns), None);
    assert_eq!(root.get_property_ns(name, "urn:p"), None);
    assert!(root.get_property_node(name).is_none());
    assert!(root.get_property_node_no_ns(name).is_none());
    assert!(root.get_property_node_ns("b", ns).is_none());
    assert!(root.get_property_node_ns(name, "urn:p").is_none());
    assert!(!root.has_property(name));
    assert!(!root.has_attribute(name));
    assert!(!root.has_property_no_ns(name));
    assert!(!root.has_property_ns("b", ns));
    assert!(!root.has_property_ns(name, "urn:p"));
    assert_eq!(root.lookup_namespace_prefix(ns), None);
    assert_eq!(root.lookup_namespace_uri(&name.replacen('a', "p", 1)), None);
  }
}

/// Both of `set_property`'s paths: before and after a value node of an attribute was wrapped (which no other test in
/// this file does).
#[test]
fn property_mutators_return_err_and_change_nothing() {
  let doc = parse();
  let mut root = doc.get_root_element().unwrap();
  let ns = root.lookup_namespace_uri("p").unwrap();
  let p = root
    .get_namespace_declarations()
    .into_iter()
    .find(|n| n.get_href() == ns)
    .unwrap();
  let check = |root: &mut Node| {
    for (name, value) in with_nul("a").iter().zip(with_nul("v").iter()) {
      assert!(root.set_property(name, "v").is_err());
      assert!(root.set_property("a", value).is_err());
      assert!(root.set_attribute(name, "v").is_err());
      assert!(root.set_property_ns(name, "v", &p).is_err());
      assert!(root.set_property_ns("b", value, &p).is_err());
      assert!(root.remove_property(name).is_err());
      assert!(root.remove_attribute(name).is_err());
      assert!(root.remove_property_no_ns(name).is_err());
      assert!(
        root
          .remove_property_ns(&name.replacen('a', "b", 1), "urn:p")
          .is_err()
      );
      assert!(
        root
          .remove_property_ns("b", &format!("urn:p{}", &name[1..]))
          .is_err()
      );
    }
    assert_eq!(root.get_property("a").as_deref(), Some("1"));
    assert_eq!(root.get_property_ns("b", "urn:p").as_deref(), Some("2"));
    assert_eq!(root.get_properties().len(), 2);
  };
  check(&mut root);
  let value_node = root
    .get_property_node("a")
    .unwrap()
    .get_first_child()
    .unwrap();
  check(&mut root);
  assert_eq!(value_node.get_content(), "1");
}

#[test]
fn node_mutators_return_err_and_change_nothing() {
  let doc = parse();
  let mut root = doc.get_root_element().unwrap();
  for s in with_nul("x") {
    assert!(root.set_name(&s).is_err());
    assert!(root.set_content(&s).is_err());
    assert!(root.new_child(None, &s).is_err());
    assert!(root.add_text_child(None, &s, "t").is_err());
    assert!(root.add_text_child(None, "t", &s).is_err());
    assert!(Namespace::new(&s, "urn:x", &mut root).is_err());
    assert!(Namespace::new("x", &s, &mut root).is_err());
  }
  assert_eq!(root.get_name(), "r");
  assert_eq!(root.get_content(), "text");
  assert_eq!(root.get_child_nodes().len(), 2);
  assert_eq!(root.lookup_namespace_uri("x"), None);
  assert_eq!(
    doc.to_string(),
    Parser::default().parse_string(XML).unwrap().to_string()
  );
}

#[test]
fn constructors_return_err() {
  let mut doc = parse();
  for s in with_nul("x") {
    assert!(Node::new(&s, None, &doc).is_err());
    assert!(Node::new_text(&s, &doc).is_err());
    assert!(doc.create_processing_instruction(&s, "data").is_err());
    assert!(doc.create_processing_instruction("pi", &s).is_err());
    assert!(doc.save_file(&s).is_err());
  }
}

#[test]
fn canonicalize_with_a_nul_in_an_inclusive_prefix_returns_err() {
  let doc = parse();
  for s in with_nul("p") {
    let options = CanonicalizationOptions {
      inclusive_ns_prefixes: vec!["p".into(), s],
      ..Default::default()
    };
    assert!(doc.canonicalize(options.clone(), None).is_err());
    assert!(
      doc
        .get_root_element()
        .unwrap()
        .canonicalize(options)
        .is_err()
    );
  }
  assert!(
    doc
      .canonicalize(CanonicalizationOptions::default(), None)
      .is_ok()
  );
}
