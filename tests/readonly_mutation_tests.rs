//! A `RoNode` read's closure may change the document, even the node it is reading: each string it is given stays
//! valid until it returns. Short strings and long ones (past the 128-byte stack copy) alike, in a parsed document (whose
//! names live in its dictionary) and in a built one (which has no dictionary). Run under AddressSanitizer to see a
//! read of freed memory; without it, the freed bytes may still read as the old string.

use libxml::parser::Parser;
use libxml::tree::{Document, Namespace, Node};

const LENGTHS: [usize; 2] = [8, 300];

/// A document built through the tree API, so without a dictionary: every name is the node's own allocation.
fn built_document(name: &str) -> (Document, Node) {
  let mut doc = Document::new().unwrap();
  let root = Node::new(name, None, &doc).unwrap();
  doc.set_root_element(&root);
  (doc, root)
}

#[test]
fn with_name_survives_renaming_the_node() {
  for length in LENGTHS {
    let name = "n".repeat(length);
    let parsed = Parser::default()
      .parse_string(format!("<{name}/>"))
      .unwrap();
    let (built, _root) = built_document(&name);
    for doc in [&parsed, &built] {
      let mut root = doc.get_root_element().unwrap();
      let read = root.as_readonly().with_name(|read| {
        root.set_name("renamed").unwrap();
        read.to_string()
      });
      assert_eq!(read, name);
      assert_eq!(root.get_name(), "renamed");
    }
  }
}

#[test]
fn with_content_survives_changing_the_content() {
  for length in LENGTHS {
    let content = "c".repeat(length);
    let doc = Parser::default()
      .parse_string(format!(
        "<r>{content}<!--{content}--><![CDATA[{content}]]><?pi {content}?></r>"
      ))
      .unwrap();
    let root = doc.get_root_element().unwrap();
    for mut node in root.get_child_nodes() {
      let read = node.as_readonly().with_content(|read| {
        node.set_content("changed").unwrap();
        read.to_string()
      });
      assert_eq!(read, content, "{:?}", node.get_type());
      assert_eq!(node.get_content(), "changed");
    }
  }
}

#[test]
fn with_namespace_survives_freeing_the_node() {
  for length in LENGTHS {
    let href = format!("urn:{}", "h".repeat(length));
    let prefix = "p".repeat(length);
    let doc = Parser::default()
      .parse_string(format!(r#"<r><{prefix}:e xmlns:{prefix}="{href}"/></r>"#))
      .unwrap();
    let root = doc.get_root_element().unwrap();
    let element = root.get_first_child().unwrap();
    let read = element.as_readonly().with_namespace_href(|read| {
      element.clone().free_subtree();
      read.map(str::to_string)
    });
    assert_eq!(read.as_deref(), Some(href.as_str()));
    let element = root.get_first_child();
    assert!(element.is_none());

    let (_built, mut root) = built_document("r");
    let ns = Namespace::new(&prefix, &href, &mut root).unwrap();
    let mut element = Node::new("e", Some(ns), &_built).unwrap();
    root.add_child(&mut element).unwrap();
    let read = element.as_readonly().with_namespace_prefix(|read| {
      element.clone().free_subtree();
      read.map(str::to_string)
    });
    assert_eq!(read.as_deref(), Some(prefix.as_str()));
  }
}

#[test]
fn for_each_property_survives_changing_and_removing_properties() {
  for length in LENGTHS {
    let name = "a".repeat(length);
    let value = "v".repeat(length);
    let parsed = Parser::default()
      .parse_string(format!(r#"<r {name}="{value}" b="{value}"/>"#))
      .unwrap();
    let (built, mut root) = built_document("r");
    root.set_property(&name, &value).unwrap();
    root.set_property("b", &value).unwrap();
    for doc in [&parsed, &built] {
      let root = doc.get_root_element().unwrap();
      let mut seen = Vec::new();
      root
        .as_readonly()
        .for_each_property(|read_name, read_value| {
          let mut element = root.clone();
          if read_name == "b" {
            element.set_property("b", "changed").unwrap();
          } else {
            element.remove_property(read_name).unwrap();
          }
          seen.push((read_name.to_string(), read_value.to_string()));
        });
      assert_eq!(
        seen,
        [
          (name.clone(), value.clone()),
          ("b".to_string(), value.clone())
        ]
      );
      assert_eq!(root.get_property("b").as_deref(), Some("changed"));
      assert!(!root.has_property(&name));
    }
  }
}

#[test]
fn for_each_property_ns_survives_changing_the_property() {
  for length in LENGTHS {
    let value = "v".repeat(length);
    let href = format!("urn:{}", "h".repeat(length));
    let doc = Parser::default()
      .parse_string(format!(r#"<r xmlns:p="{href}" p:q="{value}"/>"#))
      .unwrap();
    let root = doc.get_root_element().unwrap();
    let mut seen = Vec::new();
    root.as_readonly().for_each_property_ns(|name, ns, read| {
      root.clone().remove_property_ns(name, ns.unwrap()).unwrap();
      seen.push((name.to_string(), ns.map(str::to_string), read.to_string()));
    });
    assert_eq!(seen, [("q".to_string(), Some(href.clone()), value.clone())]);
    assert!(root.get_properties_ns().is_empty());
  }
}

#[test]
fn with_property_survives_changing_the_property() {
  for length in LENGTHS {
    let value = "v".repeat(length);
    let doc = Parser::default()
      .parse_string(format!(r#"<r a="{value}"/>"#))
      .unwrap();
    let root = doc.get_root_element().unwrap();
    let read = root.as_readonly().with_property(c"a", |read| {
      root.clone().set_property("a", "changed").unwrap();
      read.to_string()
    });
    assert_eq!(read.as_deref(), Some(value.as_str()));
    let read = root.as_readonly().with_attribute(c"a", |read| {
      root.clone().remove_property("a").unwrap();
      read.to_string()
    });
    assert_eq!(read.as_deref(), Some("changed"));
  }
}

/// A closure that removes properties the walk has not reached yet: the walk goes on from the property it showed if the
/// element still holds it, else from the next one if still held, else it stops; it never reads a removed property.
#[test]
fn for_each_property_survives_removing_later_properties() {
  let doc = Parser::default()
    .parse_string(r#"<r a="1" b="2" c="3" d="4"/>"#)
    .unwrap();
  let root = doc.get_root_element().unwrap();
  let mut seen = Vec::new();
  root.as_readonly().for_each_property(|name, _| {
    seen.push(name.to_string());
    if name == "a" {
      root.clone().remove_property("b").unwrap();
    }
  });
  assert_eq!(seen, ["a", "c", "d"]);
  // Removing the property shown and the next one ends the walk.
  seen.clear();
  root.as_readonly().for_each_property(|name, _| {
    seen.push(name.to_string());
    root.clone().remove_property(name).unwrap();
    root.clone().remove_property("c").unwrap();
  });
  assert_eq!(seen, ["a"]);
  assert_eq!(root.get_property("d").as_deref(), Some("4"));
}

/// A closure is given a copy of each name, never the document's own string, even one its dictionary holds: a short
/// copy costs less than asking the dictionary (`xmlDictOwns`), and is sound whoever owns the string.
#[test]
fn closure_reads_copy_dictionary_names() {
  let doc = Parser::default()
    .parse_string(r#"<p:r xmlns:p="urn:p" a="1" p:b="2"/>"#)
    .unwrap();
  let root = doc.get_root_readonly().unwrap();
  let element = root.node_ptr();
  let (name, ns) = unsafe { ((*element).name as usize, (*element).ns) };
  let (prefix, href) = unsafe { ((*ns).prefix as usize, (*ns).href as usize) };
  let mut property_names = Vec::new();
  let mut prop = unsafe { (*element).properties };
  while !prop.is_null() {
    property_names.push(unsafe { (*prop).name } as usize);
    prop = unsafe { (*prop).next };
  }
  assert_eq!(property_names.len(), 2);
  assert_ne!(root.with_name(|read| read.as_ptr() as usize), name);
  assert_ne!(
    root.with_namespace_prefix(|read| read.unwrap().as_ptr() as usize),
    prefix
  );
  assert_ne!(
    root.with_namespace_href(|read| read.unwrap().as_ptr() as usize),
    href
  );
  let mut seen = 0;
  root.for_each_property_ns(|read, read_href, _| {
    seen += 1;
    assert!(!property_names.contains(&(read.as_ptr() as usize)));
    if let Some(read_href) = read_href {
      assert_ne!(read_href.as_ptr() as usize, href);
    }
  });
  root.for_each_property(|read, _| {
    seen += 1;
    assert!(!property_names.contains(&(read.as_ptr() as usize)));
  });
  assert_eq!(seen, 4);
}
