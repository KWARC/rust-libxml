//! A `Node` must never keep pointing at a node libxml2 has freed (#232, #234, #235). When a safe method frees nodes,
//! or makes libxml2 free them, every cached wrapper of a freed node is neutralized, as `free_subtree` does: it reads
//! as `Node::null()` from then on. libxml2's heap is quarantined (see `common::quarantine`), so a wrapper left
//! dangling is caught by `is_freed`, without valgrind.
mod common;

use common::quarantine::{double_freed, is_freed, setup};
use libxml::parser::Parser;
use libxml::tree::{Document, Node, NodeType};

fn parse(xml: &str) -> Document {
  setup();
  Parser::default().parse_string(xml).unwrap()
}

/// Each wrapper was freed together with its node and must read as null, not point into the freed node.
fn assert_neutralized(wrappers: &[(&str, &Node)]) {
  for (what, node) in wrappers {
    assert!(
      !is_freed(node.node_ptr()),
      "the {what} wrapper points at a node libxml2 freed"
    );
    assert!(node.is_null(), "the {what} wrapper is not neutralized");
  }
}

/// #232: the documented discard (unlink, `set_rust_owned`, drop) frees the subtree in `Drop`.
#[test]
fn dropping_a_rust_owned_node_neutralizes_its_descendants() {
  let doc = parse("<r><c a='1' b='2'><d e='3'>t</d><f/>u</c><g/></r>");
  let root = doc.get_root_element().unwrap();
  let mut c = root.get_first_child().unwrap();
  let a = c.get_property_node("a").unwrap();
  let a_value = a.get_first_child().unwrap();
  let b = c.get_property_node("b").unwrap();
  let d = c.get_first_child().unwrap();
  let e = d.get_property_node("e").unwrap();
  let t = d.get_first_child().unwrap();
  let f = d.get_next_sibling().unwrap();
  let u = c.get_last_child().unwrap();
  let g = root.get_last_child().unwrap();

  c.unlink();
  c.set_rust_owned();
  drop(c);

  assert_neutralized(&[
    ("a", &a),
    ("a's value", &a_value),
    ("b", &b),
    ("d", &d),
    ("e", &e),
    ("t", &t),
    ("f", &f),
    ("u", &u),
  ]);
  assert_eq!(
    g.get_name(),
    "g",
    "a node outside the freed subtree was neutralized"
  );
}

/// #234: `xmlAddChild` merges a text node into an adjacent one and frees it.
#[test]
fn add_child_merging_text_neutralizes_the_merged_node() {
  let doc = parse("<r>hello</r>");
  let mut root = doc.get_root_element().unwrap();
  let mut text = Node::new_text("world", &doc).unwrap();

  root.add_child(&mut text).unwrap();

  assert_neutralized(&[("merged text", &text)]);
  assert_eq!(root.get_content(), "helloworld");
}

/// Before 2.13, `xmlAddNextSibling` and `xmlAddPrevSibling` merge a text node into an adjacent one too; since, they
/// insert it.
fn assert_merged_or_inserted(text: &Node, content: &str) {
  assert!(
    !is_freed(text.node_ptr()),
    "the text wrapper points at a node libxml2 freed"
  );
  assert!(
    text.is_null() || text.get_content() == content,
    "the text node was neither merged nor inserted"
  );
}

#[test]
fn add_next_sibling_merging_text_neutralizes_the_merged_node() {
  let doc = parse("<r>hello<b/></r>");
  let root = doc.get_root_element().unwrap();
  let mut hello = root.get_first_child().unwrap();
  let mut text = Node::new_text(" world", &doc).unwrap();

  hello.add_next_sibling(&mut text).unwrap();

  assert_merged_or_inserted(&text, " world");
  assert_eq!(root.get_content(), "hello world");
}

#[test]
fn add_prev_sibling_merging_text_neutralizes_the_merged_node() {
  let doc = parse("<r><b/>world</r>");
  let root = doc.get_root_element().unwrap();
  let mut world = root.get_last_child().unwrap();
  let mut text = Node::new_text("hello ", &doc).unwrap();

  world.add_prev_sibling(&mut text).unwrap();

  assert_merged_or_inserted(&text, "hello ");
  assert_eq!(root.get_content(), "hello world");
}

/// Adding an attribute node frees the element's attribute of the same name (attributes are unique).
#[test]
fn add_child_replacing_an_attribute_neutralizes_the_old_one() {
  let doc = parse("<r a='1' z='9'><s a='2'/></r>");
  let mut root = doc.get_root_element().unwrap();
  let old = root.get_property_node("a").unwrap();
  let z = root.get_property_node("z").unwrap();
  let old_value = old.get_first_child().unwrap();
  let mut new = root
    .get_first_child()
    .unwrap()
    .get_property_node("a")
    .unwrap();
  new.unlink();

  root.add_child(&mut new).unwrap();

  assert_neutralized(&[("replaced attribute", &old), ("replaced value", &old_value)]);
  assert_eq!(root.get_property("a").as_deref(), Some("2"));
  assert!(!new.is_null());
  assert_eq!(z.get_content(), "9", "the next attribute was neutralized");
}

/// #235: `xmlRemoveProp` frees the attribute and its value.
#[test]
fn remove_property_neutralizes_the_attribute() {
  let doc = parse("<r xmlns:p='urn:p' a='1' b='2' p:c='3'/>");
  let mut root = doc.get_root_element().unwrap();
  let a = root.get_property_node("a").unwrap();
  let a_value = a.get_first_child().unwrap();
  let b = root.get_property_node_no_ns("b").unwrap();
  let c = root.get_property_node_ns("c", "urn:p").unwrap();

  root.remove_property("a").unwrap();
  assert_eq!(b.get_content(), "2", "the next attribute was neutralized");
  root.remove_property_no_ns("b").unwrap();
  root.remove_property_ns("c", "urn:p").unwrap();

  assert_neutralized(&[("a", &a), ("a's value", &a_value), ("b", &b), ("c", &c)]);
  assert!(root.get_properties().is_empty());
}

/// `xmlNodeSetContent` frees an element's children, with their attributes and descendants.
#[test]
fn set_content_neutralizes_the_old_children() {
  let doc = parse("<r><a x='1'><b/></a>tail</r>");
  let mut root = doc.get_root_element().unwrap();
  let a = root.get_first_child().unwrap();
  let x = a.get_property_node("x").unwrap();
  let b = a.get_first_child().unwrap();
  let tail = root.get_last_child().unwrap();

  root.set_content("new").unwrap();

  assert_neutralized(&[("a", &a), ("x", &x), ("b", &b), ("tail", &tail)]);
  assert_eq!(root.get_content(), "new");
}

/// `xmlSetProp` keeps an existing attribute, but frees its old value.
#[test]
fn set_property_neutralizes_the_old_value() {
  let doc = parse("<r xmlns:p='urn:p' a='1' p:b='2'/>");
  let mut root = doc.get_root_element().unwrap();
  let a = root.get_property_node("a").unwrap();
  let a_value = a.get_first_child().unwrap();
  let b_value = root
    .get_property_node_ns("b", "urn:p")
    .unwrap()
    .get_first_child()
    .unwrap();

  root.set_property("a", "3").unwrap();
  // A QName: libxml2 resolves the prefix and sets the namespaced attribute.
  root.set_property("p:b", "4").unwrap();

  assert_neutralized(&[("a's old value", &a_value), ("b's old value", &b_value)]);
  assert!(!a.is_null());
  assert_eq!(a.get_content(), "3");
  assert_eq!(root.get_property_ns("b", "urn:p").as_deref(), Some("4"));
}

/// An entity reference's children belong to the entity declaration: freeing the reference leaves them alone, and so
/// must the neutralizing walk.
#[test]
fn free_subtree_leaves_the_referenced_entity_alone() {
  let doc = parse("<!DOCTYPE r [<!ENTITY e '<b>x</b>'>]><r><a>&e;</a></r>");
  let root = doc.get_root_element().unwrap();
  let a = root.get_first_child().unwrap();
  let reference = a.get_first_child().unwrap();
  assert_eq!(reference.get_type(), Some(NodeType::EntityRefNode));
  let declaration = reference.get_first_child().unwrap();
  assert_eq!(declaration.get_type(), Some(NodeType::EntityDecl));

  a.free_subtree();

  assert_neutralized(&[("entity reference", &reference)]);
  assert!(
    !declaration.is_null(),
    "the entity declaration was neutralized"
  );
  assert_eq!(declaration.get_name(), "e");
}

/// An unlinked node's wrapper is kept track of until the node is linked again, anywhere in its document: freeing it
/// there, or a node above it, neutralizes the wrapper.
#[test]
fn freeing_a_relinked_node_neutralizes_its_wrapper() {
  type Free = fn(Node, &Document);
  let paths: [(&str, Free); 3] = [
    ("set_content on its new parent", |mut parent, _| {
      parent.set_content("").unwrap()
    }),
    ("free_subtree of its new parent", |parent, _| {
      parent.free_subtree()
    }),
    ("a rust-owned drop of its new parent", |mut parent, _| {
      parent.unlink();
      parent.set_rust_owned();
    }),
  ];
  for (path, free) in paths {
    let doc = parse("<r><a><c><d/></c></a><b/></r>");
    let root = doc.get_root_element().unwrap();
    let a = root.get_first_child().unwrap();
    let mut c = a.get_first_child().unwrap();
    let d = c.get_first_child().unwrap();
    let mut b = root.get_last_child().unwrap();
    c.unlink();
    // Reached again while detached, the node has the same wrapper.
    let c_again = d.get_parent().unwrap();
    b.add_child(&mut c).unwrap();

    free(b, &doc);

    for (what, node) in [("relinked", &c), ("re-reached", &c_again), ("child", &d)] {
      assert!(
        !is_freed(node.node_ptr()),
        "{path}: the {what} wrapper points at a freed node"
      );
      assert!(
        node.is_null(),
        "{path}: the {what} wrapper is not neutralized"
      );
    }
  }
}

/// The same for an attribute, moved to another element and removed there.
#[test]
fn removing_a_moved_attribute_neutralizes_its_wrapper() {
  let doc = parse("<r><a x='1'/><b/></r>");
  let root = doc.get_root_element().unwrap();
  let a = root.get_first_child().unwrap();
  let mut b = root.get_last_child().unwrap();
  let mut x = a.get_property_node("x").unwrap();
  x.unlink();
  b.add_child(&mut x).unwrap();
  assert_eq!(b.get_property("x").as_deref(), Some("1"));

  b.remove_property("x").unwrap();

  assert_neutralized(&[("moved attribute", &x)]);
}

/// `xmlFreeDoc` frees the document's nodes and their names: every wrapper the document knows of reads as
/// `Node::null()` afterwards, and a rust-owned node is freed with it, once.
#[test]
fn dropping_the_document_neutralizes_its_wrappers() {
  let doc = parse("<r><a/><u/><o/></r>");
  let root = doc.get_root_element().unwrap();
  let a = root.get_first_child().unwrap();
  let mut u = a.get_next_sibling().unwrap();
  let mut o = root.get_last_child().unwrap();
  let new = Node::new("n", None, &doc).unwrap();
  u.unlink();
  o.unlink();
  o.set_rust_owned();
  let o_ptr = o.node_ptr();

  drop(doc);

  for (what, node) in [
    ("root", &root),
    ("linked", &a),
    ("unlinked", &u),
    ("new", &new),
  ] {
    assert!(
      !is_freed(node.node_ptr()),
      "the {what} wrapper points at a freed node"
    );
    assert!(node.is_null(), "the {what} wrapper is not neutralized");
  }
  assert!(o.is_null(), "the rust-owned wrapper is not neutralized");
  assert!(
    is_freed(o_ptr),
    "the rust-owned node was not freed with the document"
  );
  drop(o);
  assert!(!double_freed(o_ptr), "the rust-owned node was freed twice");
}

/// A node of another document is not linked: that document would free it, and its names before libxml2 2.12, while
/// its wrapper stayed with this one. `import_node` copies it instead.
#[test]
fn linking_a_node_of_another_document_is_refused() {
  let doc = parse("<r><b/></r>");
  let other = parse("<o/>");
  let mut b = doc.get_root_element().unwrap().get_first_child().unwrap();
  let mut n = Node::new("n", None, &other).unwrap();

  assert!(b.add_child(&mut n).is_err());
  assert!(b.add_prev_sibling(&mut n).is_err());
  assert!(b.add_next_sibling(&mut n).is_err());

  assert!(
    b.get_first_child().is_none()
      && b.get_prev_sibling().is_none()
      && b.get_next_sibling().is_none()
  );
  drop(other);
  assert_neutralized(&[("other document's", &n)]);
}

#[test]
#[should_panic(expected = "set_root_element called with a node of another document")]
fn set_root_element_refuses_a_node_of_another_document() {
  let mut doc = parse("<r/>");
  let other = parse("<o/>");
  let n = Node::new("n", None, &other).unwrap();
  doc.set_root_element(&n);
}
