//! A `Namespace` must not be left pointing at an `xmlNs` libxml2 has freed, nor be freed twice (#233, #235).
//! libxml2's heap is quarantined (see `common::quarantine`), so a dangling handle is caught by `is_freed` and a
//! second free by `double_freed`, without valgrind.
mod common;

use common::quarantine::{double_freed, is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::Node;
use libxml::tree::namespace::Namespace;

/// #233: `Namespace::free` freed an `xmlNs` still linked into its node's declarations, which `xmlFreeDoc` freed again.
#[test]
#[allow(deprecated)]
fn namespace_free_does_not_free_a_declared_namespace() {
  setup();
  let doc = Parser::default().parse_string("<r/>").unwrap();
  let mut root = doc.get_root_element().unwrap();
  let mut ns = Namespace::new("p", "urn:p", &mut root).unwrap();
  let ns_ptr = ns.ns_ptr();

  ns.free();
  assert!(
    !is_freed(ns_ptr),
    "free() freed a namespace its node still declares"
  );
  drop(doc);

  assert!(
    !double_freed(ns_ptr),
    "the document freed the namespace again"
  );
}

/// #235: `recursively_remove_namespaces` freed the declarations handed out by `get_namespace_declarations`.
#[test]
fn recursively_remove_namespaces_keeps_handed_out_namespaces() {
  setup();
  let doc = Parser::default()
    .parse_string("<r xmlns:p='urn:p'><p:c xmlns:q='urn:q'/></r>")
    .unwrap();
  let mut root = doc.get_root_element().unwrap();
  let declared = root.get_namespace_declarations();
  let child = root.get_first_child().unwrap();
  let used = child.get_namespace().unwrap();
  let nested = child.get_namespace_declarations();

  root.recursively_remove_namespaces().unwrap();

  for (what, ns) in [("p", &declared[0]), ("c's", &used), ("q", &nested[0])] {
    assert!(
      !is_freed(ns.ns_ptr()),
      "the {what} namespace handle points at a freed xmlNs"
    );
  }
  assert_eq!(declared[0].get_href(), "urn:p");
  assert_eq!(nested[0].get_prefix(), "q");
  assert!(root.get_namespace_declarations().is_empty());
  assert!(child.get_namespace().is_none());
  drop(doc);
  assert!(!double_freed(declared[0].ns_ptr()));
}

/// Freeing an element frees its namespace declarations, which a `Namespace`, or a node's namespace, may still point
/// at: the document keeps them instead, until it is freed itself.
#[test]
fn freeing_an_element_keeps_its_namespace_declarations() {
  setup();
  /// Frees `c`, given the root and `c`.
  type Free = fn(Node, Node);
  let frees: [(&str, Free); 4] = [
    ("free_subtree", |_, c| c.free_subtree()),
    ("set_content", |mut root, _| root.set_content("").unwrap()),
    ("a rust-owned drop", |_, mut c| {
      c.unlink();
      c.set_rust_owned();
    }),
    // libxml2 merges a node added under a text node into it, and frees the node.
    ("add_child under a text node", |root, mut c| {
      c.unlink();
      let mut text = root.get_first_child().unwrap();
      text.add_child(&mut c).unwrap();
    }),
  ];
  for (how, free) in frees {
    let doc = Parser::default()
      .parse_string("<r>t<c xmlns:p='urn:p'><p:d/></c></r>")
      .unwrap();
    let root = doc.get_root_element().unwrap();
    let c = root.get_last_child().unwrap();
    let declared = c.get_namespace_declarations();
    let used = c.get_first_child().unwrap().get_namespace().unwrap();
    let ns_ptr = declared[0].ns_ptr();

    free(root, c);

    assert!(!is_freed(ns_ptr), "{how} freed a handed-out namespace");
    assert_eq!(declared[0].get_href(), "urn:p");
    assert_eq!(used.get_prefix(), "p");
    drop(doc);
    // The namespaces keep their document alive.
    assert!(!is_freed(ns_ptr), "{how}: freed under a held namespace");
    drop((declared, used));
    assert!(is_freed(ns_ptr), "{how}: the namespace leaked");
    assert!(
      !double_freed(ns_ptr),
      "{how}: the namespace was freed twice"
    );
  }
}
