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

/// An XML and an HTML document, each `<html><body><p>t</p></body></html>`.
fn xml_and_html_documents() -> [Document; 2] {
  setup();
  let source = "<html><body><p>t</p></body></html>";
  [
    Parser::default().parse_string(source).unwrap(),
    Parser::default_html().parse_string(source).unwrap(),
  ]
}

/// The first descendant of `doc`'s root element without children: the text node `t`.
fn leaf(doc: &Document) -> Node {
  let mut node = doc.get_root_element().unwrap();
  while let Some(child) = node.get_first_child() {
    node = child;
  }
  assert!(node.is_text_node());
  node
}

/// `free_subtree` and `unlink_node` leave a document's own node alone, an HTML one as an XML one: `xmlFreeNode` would
/// free it as an ordinary node, and its `Document` free it again.
#[test]
fn freeing_a_document_node_is_refused() {
  for doc in xml_and_html_documents() {
    let doc_ptr = doc.doc_ptr();
    let t = leaf(&doc);

    doc.as_node().free_subtree();
    assert!(!is_freed(doc_ptr), "free_subtree freed a document");
    assert_eq!(t.get_content(), "t");

    let mut doc_node = doc.as_node();
    doc_node.unlink();
    assert!(
      !doc_node.is_unlinked(),
      "a document's node was marked unlinked, which set_rust_owned would free"
    );

    drop(doc);
    assert!(!double_freed(doc_ptr), "the document was freed twice");
  }
}

/// Whether `link` refused to link `doc`'s own node. A document it did link is leaked, not freed: freeing it would free
/// it twice, or recurse without end.
fn refuses_the_document(
  doc: Document,
  link: impl FnOnce(&Document, &mut Node) -> Result<(), String>,
) -> bool {
  let refused = link(&doc, &mut doc.as_node()).is_err();
  if !refused {
    std::mem::forget(doc);
  }
  refused
}

/// A document is not linked into its own tree: libxml2 frees one added under a text node at once, as an ordinary node,
/// and `xmlFreeDoc` recurses through one linked elsewhere.
#[test]
fn linking_a_document_is_refused() {
  type Link = fn(&Document, &mut Node) -> Result<(), String>;
  let links: [(&str, Link); 4] = [
    ("add_child under an element", |doc, d| {
      doc.get_root_element().unwrap().add_child(d)
    }),
    ("add_child under a text node", |doc, d| {
      leaf(doc).add_child(d)
    }),
    ("add_prev_sibling", |doc, d| {
      let mut body = doc.get_root_element().unwrap().get_last_child().unwrap();
      body.add_prev_sibling(d).map_err(|e| e.to_string())
    }),
    ("add_next_sibling", |doc, d| {
      let mut body = doc.get_root_element().unwrap().get_last_child().unwrap();
      body.add_next_sibling(d).map_err(|e| e.to_string())
    }),
  ];
  let mut linked = Vec::new();
  for (what, link) in links {
    for doc in xml_and_html_documents() {
      let kind = match doc.as_node().get_type() {
        Some(NodeType::HtmlDocumentNode) => "an HTML",
        _ => "an XML",
      };
      if !refuses_the_document(doc, link) {
        linked.push(format!("{what} linked {kind} document"));
      }
    }
  }
  assert!(linked.is_empty(), "{linked:#?}");
}

/// [`linking_a_document_is_refused`], by `set_root_element`.
#[test]
fn set_root_element_refuses_a_document() {
  for mut doc in xml_and_html_documents() {
    let doc_node = doc.as_node();
    let set = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      doc.set_root_element(&doc_node)
    }));
    if set.is_ok() {
      std::mem::forget(doc); // its tree contains it now: freeing it would recurse without end
      panic!("set_root_element linked a document");
    }
  }
}

/// Whether `link` refused to link one of `doc`'s nodes. A document it did link a node in is leaked, not freed: its tree
/// may loop.
fn refuses(doc: Document, link: impl FnOnce(&Document) -> Result<(), String>) -> bool {
  let refused = link(&doc).is_err();
  if !refused {
    std::mem::forget(doc);
  }
  refused
}

/// The node at `path` (child indices from the root element) in `doc`.
fn at(doc: &Document, path: &[usize]) -> Node {
  let mut node = doc.get_root_element().unwrap();
  for &index in path {
    node = node.get_child_nodes().swap_remove(index);
  }
  node
}

/// A node is not linked under itself or one of its descendants: libxml2 makes a loop of them, which every walk up or
/// down the tree then follows without end.
#[test]
fn linking_a_node_under_itself_is_refused() {
  type Link = fn(&Document) -> Result<(), String>;
  // <r><a><b><c/></b></a></r>: a is [0], b [0, 0] and c [0, 0, 0].
  let links: [(&str, Link); 6] = [
    ("add_child of a grandparent", |doc| {
      at(doc, &[0, 0, 0]).add_child(&mut at(doc, &[0]))
    }),
    ("add_child of a parent", |doc| {
      at(doc, &[0, 0, 0]).add_child(&mut at(doc, &[0, 0]))
    }),
    ("add_prev_sibling of a grandparent", |doc| {
      at(doc, &[0, 0, 0])
        .add_prev_sibling(&mut at(doc, &[0]))
        .map_err(|e| e.to_string())
    }),
    ("add_next_sibling of a parent", |doc| {
      at(doc, &[0, 0, 0])
        .add_next_sibling(&mut at(doc, &[0, 0]))
        .map_err(|e| e.to_string())
    }),
    ("add_child of itself", |doc| {
      at(doc, &[0]).add_child(&mut at(doc, &[0]))
    }),
    ("add_prev_sibling of itself", |doc| {
      at(doc, &[0, 0, 0])
        .add_prev_sibling(&mut at(doc, &[0, 0, 0]))
        .map_err(|e| e.to_string())
    }),
  ];
  let mut linked = Vec::new();
  for (what, link) in links {
    let doc = parse("<r><a><b><c/></b></a></r>");
    if refuses(doc, link) {
      let doc = parse("<r><a><b><c/></b></a></r>");
      let _ = link(&doc);
      assert_eq!(
        doc.node_to_string(&doc.get_root_element().unwrap()),
        "<r><a><b><c/></b></a></r>",
        "{what} changed the tree"
      );
    } else {
      linked.push(what);
    }
  }
  assert!(linked.is_empty(), "{linked:#?}");
}

/// The attribute `a` of `doc`'s root element, unlinked.
fn unlinked_attribute(doc: &Document) -> Node {
  let mut a = doc
    .get_root_element()
    .unwrap()
    .get_property_node("a")
    .unwrap();
  a.unlink();
  a
}

/// An attribute is added to an element only, as libxml2 required before 2.13. Since, `xmlAddChild` adds one to any
/// node's attribute list, at a field that a document's external subset overlays, or a DTD's attribute declarations,
/// and merges one added to a text node into it, reading its type as its text: an ID attribute's is the address 2.
#[test]
fn adding_an_attribute_outside_an_element_is_refused() {
  type Link = fn(&Document) -> Result<(), String>;
  let links: [(&str, Link); 3] = [
    ("a document", |doc| {
      doc.as_node().add_child(&mut unlinked_attribute(doc))
    }),
    ("a text node", |doc| {
      at(doc, &[0]).add_child(&mut unlinked_attribute(doc))
    }),
    ("a comment", |doc| {
      at(doc, &[1]).add_child(&mut unlinked_attribute(doc))
    }),
  ];
  let mut added = Vec::new();
  for (what, link) in links {
    if !refuses(parse("<r a='1'>t<!--c--></r>"), link) {
      added.push(format!("an attribute was added to {what}"));
    }
  }
  assert!(added.is_empty(), "{added:#?}");
}

/// `add_child` moves a linked node: it unlinks it first, as libxml2 does since 2.13. Before, `xmlAddChild` left it in
/// its old parent's children as well, which then looped, or kept it after it was merged into a text node and freed.
#[test]
fn add_child_moves_a_linked_node() {
  let doc = parse("<r><a/><b/><c/></r>");
  let mut a = at(&doc, &[0]);
  at(&doc, &[2]).add_child(&mut a).unwrap();
  if at(&doc, &[0]) == a || a.get_next_sibling().is_some() {
    std::mem::forget(doc); // its tree loops: serializing or freeing it does not end
    panic!("add_child left the moved node in its old parent's children");
  }
  assert_eq!(
    doc.node_to_string(&doc.get_root_element().unwrap()),
    "<r><b/><c><a/></c></r>"
  );
}

/// [`add_child_moves_a_linked_node`], into a text node it is merged into: before libxml2 2.13 it stayed in its old
/// parent's children after it was freed.
#[test]
fn add_child_merging_a_linked_text_node_unlinks_it() {
  let doc = parse("<r><p>x</p><q>y</q></r>");
  let p = at(&doc, &[0]);
  let mut x = p.get_first_child().unwrap();
  at(&doc, &[1]).add_child(&mut x).unwrap();
  let left = p.get_first_child();
  if left.as_ref().is_some_and(|x| is_freed(x.node_ptr())) {
    std::mem::forget(doc);
    panic!("the merged text node was freed, but left in its old parent");
  }
  assert!(left.is_none());
  assert_eq!(
    doc.node_to_string(&doc.get_root_element().unwrap()),
    "<r><p/><q>yx</q></r>"
  );
}

/// A node that libxml2 did not insert stays, or becomes, unlinked, as it is: `set_rust_owned` can free it. libxml2
/// inserts an attribute next to attributes only, and before 2.13 unlinks one before it refuses it.
#[test]
fn a_node_not_inserted_is_left_unlinked() {
  let doc = parse("<r a='1' b='2'><c/></r>");
  let mut c = at(&doc, &[0]);
  let mut a = unlinked_attribute(&doc);
  let mut b = doc
    .get_root_element()
    .unwrap()
    .get_property_node("b")
    .unwrap();
  let mut n = Node::new("n", None, &doc).unwrap();

  assert!(c.add_next_sibling(&mut a).is_err());
  assert!(c.add_prev_sibling(&mut a).is_err());
  assert!(c.add_next_sibling(&mut b).is_err());
  assert!(n.add_child(&mut n.clone()).is_err());

  assert!(
    a.is_unlinked() && n.is_unlinked(),
    "a node not inserted is marked linked"
  );
  let detached = b.get_parent().is_none();
  assert_eq!(
    b.is_unlinked(),
    detached,
    "b is marked linked: {}",
    !detached
  );
  for node in [a, n] {
    node.set_rust_owned();
  }
}

/// [`linking_a_node_under_itself_is_refused`], under one of its attributes: they are under it too, though it may have
/// no children.
#[test]
fn linking_an_element_under_its_own_attribute_is_refused() {
  type Link = fn(&Document) -> Result<(), String>;
  fn attribute(doc: &Document) -> Node {
    at(doc, &[0]).get_property_node("a").unwrap()
  }
  // <r><e a='v'/></r>: e is [0].
  let links: [(&str, Link); 4] = [
    ("add_child of an element to its attribute", |doc| {
      attribute(doc).add_child(&mut at(doc, &[0]))
    }),
    ("add_child of an element to its attribute's value", |doc| {
      attribute(doc)
        .get_first_child()
        .unwrap()
        .add_child(&mut at(doc, &[0]))
    }),
    (
      "add_next_sibling of an element to its attribute's value",
      |doc| {
        attribute(doc)
          .get_first_child()
          .unwrap()
          .add_next_sibling(&mut at(doc, &[0]))
          .map_err(|e| e.to_string())
      },
    ),
    ("add_child of a childless element to itself", |doc| {
      at(doc, &[0]).add_child(&mut at(doc, &[0]))
    }),
  ];
  let mut linked = Vec::new();
  for (what, link) in links {
    if refuses(parse("<r><e a='v'/></r>"), link) {
      let doc = parse("<r><e a='v'/></r>");
      let _ = link(&doc);
      assert_eq!(
        doc.node_to_string(&doc.get_root_element().unwrap()),
        "<r><e a=\"v\"/></r>",
        "{what} changed the tree"
      );
    } else {
      linked.push(what);
    }
  }
  assert!(linked.is_empty(), "{linked:#?}");
}

/// A node without a parent takes no sibling: nothing would free the sibling with it. `xmlFreeNode` frees a node, not
/// its siblings, so a rust-owned node's drop left its sibling pointing at it, and `add_child` moved a node out of its
/// parent only, leaving it in such a chain, which libxml2 then freed twice.
#[test]
fn a_sibling_of_a_node_without_a_parent_is_refused() {
  let doc = parse("<r><p/></r>");
  let mut detached = Node::new("d", None, &doc).unwrap();
  let mut owned = Node::new("o", None, &doc).unwrap();
  owned.set_rust_owned();
  let mut linked = Vec::new();
  for (what, anchor) in [
    ("a detached node", &mut detached),
    ("a rust-owned node", &mut owned),
  ] {
    let mut next = Node::new("n", None, &doc).unwrap();
    let mut prev = Node::new("p", None, &doc).unwrap();
    if anchor.add_next_sibling(&mut next).is_ok() {
      linked.push(format!("add_next_sibling of {what}"));
    }
    if anchor.add_prev_sibling(&mut prev).is_ok() {
      linked.push(format!("add_prev_sibling of {what}"));
    }
    if !linked.is_empty() {
      std::mem::forget(doc);
      panic!("{linked:#?}");
    }
    assert!(anchor.get_next_sibling().is_none() && anchor.get_prev_sibling().is_none());
    for node in [next, prev] {
      assert!(node.is_unlinked());
      node.set_rust_owned();
    }
  }
}

/// An attribute's siblings are attributes, and a child's are not: libxml2 refuses an attribute next to a child (and
/// before 2.13 unlinks it first), but links a child into an element's attributes. `xmlFreeProp` then freed it as an
/// attribute, its static name too, and since 2.13 `add_prev_sibling` made it the element's first child as well, so the
/// attribute after it was freed twice.
#[test]
fn an_attributes_siblings_are_attributes() {
  type Link = fn(&Document) -> Result<(), String>;
  fn attribute(doc: &Document) -> Node {
    doc
      .get_root_element()
      .unwrap()
      .get_property_node("a")
      .unwrap()
  }
  // <r a='1'><c/></r>: c is [0].
  let links: [(&str, Link); 5] = [
    ("a text node after an attribute", |doc| {
      let mut text = Node::new_text("t", doc).unwrap();
      attribute(doc)
        .add_next_sibling(&mut text)
        .map_err(|e| e.to_string())
    }),
    ("a text node before an attribute", |doc| {
      let mut text = Node::new_text("t", doc).unwrap();
      attribute(doc)
        .add_prev_sibling(&mut text)
        .map_err(|e| e.to_string())
    }),
    ("an element after an attribute", |doc| {
      attribute(doc)
        .add_next_sibling(&mut at(doc, &[0]))
        .map_err(|e| e.to_string())
    }),
    ("an attribute after an element", |doc| {
      at(doc, &[0])
        .add_next_sibling(&mut attribute(doc))
        .map_err(|e| e.to_string())
    }),
    ("an attribute before an element", |doc| {
      at(doc, &[0])
        .add_prev_sibling(&mut attribute(doc))
        .map_err(|e| e.to_string())
    }),
  ];
  let mut linked = Vec::new();
  for (what, link) in links {
    if refuses(parse("<r a='1'><c/></r>"), link) {
      let doc = parse("<r a='1'><c/></r>");
      let _ = link(&doc);
      assert_eq!(
        doc.node_to_string(&doc.get_root_element().unwrap()),
        "<r a=\"1\"><c/></r>",
        "linking {what} changed the tree"
      );
    } else {
      linked.push(what);
    }
  }
  assert!(linked.is_empty(), "{linked:#?}");
}

/// `set_root_element` links no attribute: libxml2 makes it a child of the document, which `xmlUnlinkNode` does not
/// take it out of (it unlinks an attribute from its parent's attributes), so the document freed it again after it was
/// moved and removed.
#[test]
fn set_root_element_refuses_an_attribute() {
  let mut doc = parse("<r a='1'/>");
  let mut root = doc.get_root_element().unwrap();
  let a = root.get_property_node("a").unwrap();
  root.unlink_node();
  let set = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| doc.set_root_element(&a)));
  if set.is_ok() {
    std::mem::forget(doc);
    panic!("set_root_element linked an attribute");
  }
  assert_eq!(a.get_parent(), Some(root));
}

/// `set_root_element` of a text node, in a document with no element: libxml2 appends it to the document's children,
/// and merges it into a text node there and frees it, as `add_next_sibling` does.
#[test]
fn set_root_element_merging_text_neutralizes_the_merged_node() {
  setup();
  let mut doc = Document::new().unwrap();
  let first = Node::new_text("a", &doc).unwrap();
  doc.set_root_element(&first);
  let second = Node::new_text("b", &doc).unwrap();
  doc.set_root_element(&second);
  assert_neutralized(&[("merged text", &second)]);
  assert!(!first.is_null() && !first.is_unlinked());
  assert_eq!(first.get_content(), "ab");
}

/// `add_child` of a parent's last child leaves it in place, as libxml2 does since 2.13: moving it after its previous
/// sibling merged it into a text node there, and freed it.
#[test]
fn add_child_of_the_last_child_leaves_it_in_place() {
  let doc = parse("<r>a<!--c-->b</r>");
  let mut r = doc.get_root_element().unwrap();
  at(&doc, &[1]).unlink_node(); // a and b are adjacent text nodes now
  let mut b = r.get_last_child().unwrap();
  r.add_child(&mut b).unwrap();
  assert!(!b.is_null(), "re-adding the last child freed it");
  assert!(!b.is_unlinked());
  assert_eq!(r.get_child_nodes().len(), 2);
  assert_eq!(b.get_content(), "b");
}

/// `dup_node_into_new_doc` makes its copy the new document's root with `xmlDocSetRootElement`, so it refuses what
/// `set_root_element` refuses: an attribute's copy became a child of the new document, which `xmlUnlinkNode` does not
/// take it out of, so the document freed it again after it was moved and removed; and a document's copy was linked
/// into the new document's tree.
#[test]
fn dup_node_into_new_doc_refuses_an_attribute_or_a_document() {
  let doc = parse("<r a='1'/>");
  let attribute = doc
    .get_root_element()
    .unwrap()
    .get_property_node("a")
    .unwrap();
  if let Ok(new) = Document::dup_node_into_new_doc(&attribute) {
    let mut copy = new.as_node().get_first_child().unwrap();
    let copy_ptr = copy.node_ptr();
    let mut x = Node::new("x", None, &new).unwrap();
    new.as_node().add_child(&mut x).unwrap();
    x.add_child(&mut copy).unwrap();
    x.remove_property("a").unwrap();
    drop((x, copy, new));
    assert!(
      !double_freed(copy_ptr),
      "the attribute's copy was freed twice"
    );
    panic!("dup_node_into_new_doc made an attribute's copy a document's child");
  }
  for doc in xml_and_html_documents() {
    assert!(
      Document::dup_node_into_new_doc(&doc.as_node()).is_err(),
      "dup_node_into_new_doc linked a copy of {:?}",
      doc.as_node().get_type()
    );
  }
}
