//! `Node::with_property` reads what `Node::get_property` reads, without copying the value.

use libxml::parser::Parser;

/// Every shape of value reads the same through both accessors: plain, empty, absent, a predefined entity (`&amp;`, one text node), an
/// unsubstituted entity reference (the copying fallback), a DTD default (the fallback), a namespaced attribute.
#[test]
fn with_property_matches_get_property() {
  let xml = r#"<!DOCTYPE r [<!ENTITY e "EE"><!ATTLIST r d CDATA "dflt">]>
<r a="1234567890123456789" empty="" amp="x&amp;y" ent="x&e;y" xmlns:p="urn:p" p:q="nsval"/>"#;
  let doc = Parser::default().parse_string(xml).unwrap();
  let root = doc.get_root_element().unwrap();
  for name in ["a", "empty", "amp", "ent", "d", "q", "absent"] {
    let c_name = std::ffi::CString::new(name).unwrap();
    assert_eq!(
      root.with_property(&c_name, |value| value.to_string()),
      root.get_property(name),
      "property {name:?}"
    );
    assert_eq!(
      root.with_attribute(&c_name, str::len),
      root.get_attribute(name).map(|value| value.len())
    );
  }
  assert_eq!(
    root.with_property(c"a", |v| v.parse::<u64>().ok()),
    Some(Some(1234567890123456789))
  );
  assert_eq!(
    root.with_property(c"ent", str::to_string).as_deref(),
    Some("xEEy")
  );
  // (a DTD default is read through the copying fallback)
  assert_eq!(
    root.with_property(c"d", str::to_string).as_deref(),
    Some("dflt")
  );
}

/// While the value is being read, the node cannot be changed through another handle.
#[test]
fn with_property_refuses_a_change_while_reading() {
  let doc = Parser::default().parse_string(r#"<r a="v"/>"#).unwrap();
  let root = doc.get_root_element().unwrap();
  let mut other = root.clone();
  let changed = root.with_property(c"a", |value| {
    let attempt = other.set_property("a", "changed");
    (value.to_string(), attempt.is_err())
  });
  assert_eq!(changed, Some(("v".to_string(), true)));
  assert_eq!(root.get_property("a").as_deref(), Some("v"));
}
