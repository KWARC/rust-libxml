//! `RoNode`'s name-taking reads pass the name to libxml2 as `Node`'s do: copied on the stack when shorter than 128
//! bytes, through a `CString` otherwise, and read what `Node`'s read on both sides of that boundary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use libxml::parser::Parser;
use libxml::readonly::RoNode;
use libxml::tree::{Document, Node};

/// The system allocator, counting the Rust heap allocations made on each thread.
struct Counting;

thread_local! {
  static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    unsafe { System.alloc(layout) }
  }
  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    unsafe { System.dealloc(ptr, layout) }
  }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// How many Rust heap allocations `op` makes on this thread, its result's included.
fn allocations<R>(op: impl FnOnce() -> R) -> usize {
  let before = ALLOCATIONS.with(Cell::get);
  drop(std::hint::black_box(op()));
  ALLOCATIONS.with(Cell::get) - before
}

fn name_of(length: usize) -> String {
  format!("n{}", "x".repeat(length - 1))
}

/// A root holding a property for each name length around the stack copy's 128-byte limit, in no namespace and in
/// `urn:p`, with a namespace prefix of each length in scope.
fn boundary_document() -> Document {
  let mut attributes = String::new();
  for length in [1, 127, 128, 300] {
    let name = name_of(length);
    let prefix = format!("p{}", "x".repeat(length - 1));
    attributes.push_str(&format!(
      r#" {name}="v{length}" xmlns:{prefix}="urn:{length}" {prefix}:{name}="ns{length}""#
    ));
  }
  Parser::default()
    .parse_string(format!("<r{attributes}/>"))
    .unwrap()
}

/// Each name-taking read answers as `Node`'s does, for names of 1, 127, 128 and 300 bytes, absent names and the empty
/// name.
#[test]
fn ronode_name_reads_match_node_reads() {
  let doc = boundary_document();
  let root = doc.get_root_element().unwrap();
  let ro = root.as_readonly();
  let node_ptr = |node: Option<Node>| node.map(|node| node.node_ptr());
  let ro_ptr = |node: Option<RoNode>| node.map(|node| node.node_ptr());
  let mut names: Vec<String> = [1, 127, 128, 300].into_iter().map(name_of).collect();
  names.extend(["absent".to_string(), name_of(200), String::new()]);
  for name in &names {
    assert_eq!(ro.get_property(name), root.get_property(name), "{name:?}");
    assert_eq!(ro.get_attribute(name), root.get_attribute(name));
    assert_eq!(ro.get_property_no_ns(name), root.get_property_no_ns(name));
    assert_eq!(ro.get_attribute_no_ns(name), root.get_attribute_no_ns(name));
    assert_eq!(ro.has_property(name), root.has_property(name));
    assert_eq!(ro.has_attribute(name), root.has_attribute(name));
    assert_eq!(ro.has_property_no_ns(name), root.has_property_no_ns(name));
    assert_eq!(ro.has_attribute_no_ns(name), root.has_attribute_no_ns(name));
    assert_eq!(
      ro_ptr(ro.get_property_node(name)),
      node_ptr(root.get_property_node(name))
    );
    assert_eq!(
      ro_ptr(ro.get_attribute_node(name)),
      node_ptr(root.get_attribute_node(name))
    );
    assert_eq!(
      ro_ptr(ro.get_property_node_no_ns(name)),
      node_ptr(root.get_property_node_no_ns(name))
    );
    assert_eq!(
      ro_ptr(ro.get_attribute_node_no_ns(name)),
      node_ptr(root.get_attribute_node_no_ns(name))
    );
    for length in [1, 127, 128, 300] {
      let ns = format!("urn:{length}");
      assert_eq!(
        ro.get_property_ns(name, &ns),
        root.get_property_ns(name, &ns),
        "{name:?} in {ns}"
      );
      assert_eq!(
        ro.get_attribute_ns(name, &ns),
        root.get_attribute_ns(name, &ns)
      );
      assert_eq!(
        ro.has_property_ns(name, &ns),
        root.has_property_ns(name, &ns)
      );
      assert_eq!(
        ro.has_attribute_ns(name, &ns),
        root.has_attribute_ns(name, &ns)
      );
      assert_eq!(
        ro_ptr(ro.get_property_node_ns(name, &ns)),
        node_ptr(root.get_property_node_ns(name, &ns))
      );
      assert_eq!(
        ro_ptr(ro.get_attribute_node_ns(name, &ns)),
        node_ptr(root.get_attribute_node_ns(name, &ns))
      );
    }
  }
  // Each property is found, in its namespace and out of it.
  for length in [1, 127, 128, 300] {
    let name = name_of(length);
    let ns = format!("urn:{length}");
    assert_eq!(ro.get_property(&name), Some(format!("v{length}")));
    assert_eq!(ro.get_property_ns(&name, &ns), Some(format!("ns{length}")));
    let prefix = format!("p{}", "x".repeat(length - 1));
    assert_eq!(ro.lookup_namespace_prefix(&ns), Some(prefix.clone()));
    assert_eq!(
      ro.lookup_namespace_prefix(&ns),
      root.lookup_namespace_prefix(&ns)
    );
    assert_eq!(ro.lookup_namespace_uri(&prefix), Some(ns.clone()));
    assert_eq!(
      ro.lookup_namespace_uri(&prefix),
      root.lookup_namespace_uri(&prefix)
    );
  }
  assert_eq!(ro.lookup_namespace_prefix(""), None);
  assert_eq!(ro.lookup_namespace_uri(""), None);
  assert_eq!(ro.lookup_namespace_prefix("urn:absent"), None);
  assert_eq!(ro.lookup_namespace_uri("absent"), None);
}

/// A read to run under `catch_unwind`.
type Read<'a> = Box<dyn Fn() + 'a>;

/// A name or namespace holding a NUL byte panics in every name-taking read, as in `Node`'s (`CString::new(..)
/// .unwrap()`), whatever its length.
#[test]
fn ronode_name_reads_panic_on_a_nul_in_the_name() {
  let doc = boundary_document();
  let ro = doc.get_root_readonly().unwrap();
  let long = format!("a\0{}", "x".repeat(200));
  for name in ["a\0b", long.as_str()] {
    let reads: [(&str, Read); 12] = [
      (
        "get_property",
        Box::new(|| {
          let _ = ro.get_property(name);
        }),
      ),
      (
        "get_property_ns",
        Box::new(|| {
          let _ = ro.get_property_ns("a", name);
        }),
      ),
      (
        "get_property_no_ns",
        Box::new(|| {
          let _ = ro.get_property_no_ns(name);
        }),
      ),
      (
        "get_property_node",
        Box::new(|| {
          let _ = ro.get_property_node(name);
        }),
      ),
      (
        "get_property_node_ns",
        Box::new(|| {
          let _ = ro.get_property_node_ns(name, "urn:1");
        }),
      ),
      (
        "get_property_node_no_ns",
        Box::new(|| {
          let _ = ro.get_property_node_no_ns(name);
        }),
      ),
      (
        "has_property",
        Box::new(|| {
          let _ = ro.has_property(name);
        }),
      ),
      (
        "has_property_ns",
        Box::new(|| {
          let _ = ro.has_property_ns(name, "urn:1");
        }),
      ),
      (
        "has_property_no_ns",
        Box::new(|| {
          let _ = ro.has_property_no_ns(name);
        }),
      ),
      (
        "get_attribute",
        Box::new(|| {
          let _ = ro.get_attribute(name);
        }),
      ),
      (
        "lookup_namespace_prefix",
        Box::new(|| {
          let _ = ro.lookup_namespace_prefix(name);
        }),
      ),
      (
        "lookup_namespace_uri",
        Box::new(|| {
          let _ = ro.lookup_namespace_uri(name);
        }),
      ),
    ];
    for (read, call) in reads {
      let panic = catch_unwind(AssertUnwindSafe(call)).expect_err(read);
      let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .unwrap_or_default();
      assert!(message.contains("NulError"), "{read}: {message}");
    }
  }
}

/// A name shorter than 128 bytes reaches libxml2 without a heap allocation: the reads that return no `String`
/// allocate nothing, those that do allocate only their result. A 128-byte name takes the `CString`.
#[test]
fn ronode_name_reads_do_not_allocate_for_short_names() {
  let doc = boundary_document();
  let ro = doc.get_root_readonly().unwrap();
  for length in [1, 127] {
    let name = name_of(length);
    let ns = format!("urn:{length}");
    let prefix = format!("p{}", "x".repeat(length - 1));
    let counts = [
      ("has_property", allocations(|| ro.has_property(&name))),
      (
        "has_property_ns",
        allocations(|| ro.has_property_ns(&name, &ns)),
      ),
      (
        "has_property_no_ns",
        allocations(|| ro.has_property_no_ns(&name)),
      ),
      ("has_attribute", allocations(|| ro.has_attribute(&name))),
      (
        "get_property_node",
        allocations(|| ro.get_property_node(&name)),
      ),
      (
        "get_property_node_ns",
        allocations(|| ro.get_property_node_ns(&name, &ns)),
      ),
      (
        "get_property_node_no_ns",
        allocations(|| ro.get_property_node_no_ns(&name)),
      ),
      (
        "get_attribute_node",
        allocations(|| ro.get_attribute_node(&name)),
      ),
      ("get_property", allocations(|| ro.get_property(&name)) - 1),
      (
        "get_property_ns",
        allocations(|| ro.get_property_ns(&name, &ns)) - 1,
      ),
      (
        "get_property_no_ns",
        allocations(|| ro.get_property_no_ns(&name)) - 1,
      ),
      ("get_attribute", allocations(|| ro.get_attribute(&name)) - 1),
      (
        "lookup_namespace_prefix",
        allocations(|| ro.lookup_namespace_prefix(&ns)) - 1,
      ),
      (
        "lookup_namespace_uri",
        allocations(|| ro.lookup_namespace_uri(&prefix)) - 1,
      ),
    ];
    let allocating: Vec<_> = counts.iter().filter(|(_, count)| *count != 0).collect();
    assert!(
      allocating.is_empty(),
      "{length}-byte name, allocations beyond the result: {allocating:?}"
    );
  }
  let name = name_of(128);
  assert_eq!(allocations(|| ro.has_property(&name)), 1);
  assert_eq!(allocations(|| ro.get_property(&name)), 2);
}

/// `Node::lookup_namespace_prefix` / `lookup_namespace_uri` pass a name shorter than 128 bytes to libxml2 as `RoNode`'s
/// do, allocating only their result, and answer as `RoNode`'s for 1-, 127-, 128- and 300-byte names and a missing one.
#[test]
fn node_namespace_lookups_do_not_allocate_for_short_names() {
  let doc = boundary_document();
  let node = doc.get_root_element().unwrap();
  let ro = doc.get_root_readonly().unwrap();
  for length in [1, 127] {
    let ns = format!("urn:{length}");
    let prefix = format!("p{}", "x".repeat(length - 1));
    assert_eq!(allocations(|| node.lookup_namespace_prefix(&ns)), 1);
    assert_eq!(allocations(|| node.lookup_namespace_uri(&prefix)), 1);
    assert_eq!(allocations(|| node.lookup_namespace_uri("absent")), 0);
  }
  for length in [1, 127, 128, 300] {
    let ns = format!("urn:{length}");
    let prefix = format!("p{}", "x".repeat(length - 1));
    assert_eq!(node.lookup_namespace_prefix(&ns), Some(prefix.clone()));
    assert_eq!(
      node.lookup_namespace_prefix(&ns),
      ro.lookup_namespace_prefix(&ns)
    );
    assert_eq!(node.lookup_namespace_uri(&prefix), Some(ns));
    assert_eq!(
      node.lookup_namespace_uri(&prefix),
      ro.lookup_namespace_uri(&prefix)
    );
  }
  assert_eq!(node.lookup_namespace_prefix("urn:absent"), None);
  assert_eq!(node.lookup_namespace_uri("absent"), None);
}

/// `RoNode::with_property` reads what `Node::with_property` reads, for every shape of value (plain, empty, an entity,
/// an unsubstituted entity reference, a DTD default, a namespaced property, values at and past 128 bytes) and names of
/// 1, 127, 128 and 300 bytes, and allocates nothing for a short value.
#[test]
fn ronode_with_property_matches_node_with_property() {
  let xml = format!(
    r#"<!DOCTYPE r [<!ENTITY e "EE"><!ATTLIST r d CDATA "dflt">]>
<r a="12345" empty="" amp="x&amp;y" ent="x&e;y" xmlns:p="urn:p" p:q="nsval" b128="{}" b129="{}" {}="1" {}="127" {}="128" {}="300"/>"#,
    "s".repeat(128),
    "h".repeat(129),
    name_of(1).replace('n', "m"),
    name_of(127),
    name_of(128),
    name_of(300)
  );
  let doc = Parser::default().parse_string(xml).unwrap();
  let root = doc.get_root_element().unwrap();
  let ro = root.as_readonly();
  let mut names: Vec<String> = [
    "a", "empty", "amp", "ent", "d", "q", "b128", "b129", "absent", "", "m",
  ]
  .map(String::from)
  .to_vec();
  names.extend([127, 128, 300].map(name_of));
  for name in &names {
    let c_name = std::ffi::CString::new(name.as_str()).unwrap();
    let expected = root.with_property(&c_name, str::to_string);
    assert_eq!(expected, root.get_property(name), "{name:?}");
    assert_eq!(
      ro.with_property(&c_name, str::to_string),
      expected,
      "{name:?}"
    );
    assert_eq!(
      ro.with_attribute(&c_name, str::len),
      expected.map(|value| value.len())
    );
  }
  assert_eq!(
    ro.with_property(c"ent", str::to_string).as_deref(),
    Some("xEEy")
  );
  assert_eq!(
    ro.with_property(c"d", str::to_string).as_deref(),
    Some("dflt")
  );
  assert_eq!(
    ro.with_property(c"a", |value| value.parse::<u32>().ok()),
    Some(Some(12345))
  );
  assert_eq!(allocations(|| ro.with_property(c"a", str::len)), 0);
  assert_eq!(allocations(|| ro.with_property(c"absent", str::len)), 0);
  // A null node, and a node that is not an element, have no property.
  assert_eq!(RoNode::null().with_property(c"a", str::len), None);
  let text = Node::new_text("a", &doc).unwrap();
  assert_eq!(text.as_readonly().with_property(c"a", str::len), None);
}

/// `for_each_property` does not read the properties' namespace URIs, which only `for_each_property_ns` passes: an
/// invalid-UTF-8 URI (decoded through an allocation) costs it nothing, and short names and values allocate nothing.
#[test]
fn for_each_property_reads_no_namespace_uri() {
  use libxml::bindings::{xmlNewNs, xmlSetNsProp};
  let doc = Parser::default().parse_string(r#"<r a="1"/>"#).unwrap();
  let root = doc.get_root_element().unwrap();
  unsafe {
    let ns = xmlNewNs(
      root.node_ptr(),
      c"urn:\xff".as_ptr().cast(),
      c"p".as_ptr().cast(),
    );
    xmlSetNsProp(
      root.node_ptr(),
      ns,
      c"q".as_ptr().cast(),
      c"w".as_ptr().cast(),
    );
  }
  let ro = root.as_readonly();
  let mut seen = 0;
  assert_eq!(allocations(|| ro.for_each_property(|_, _| seen += 1)), 0);
  assert_eq!(seen, 2);
  let mut uris = Vec::new();
  ro.for_each_property_ns(|_, ns, _| uris.push(ns.map(str::to_string)));
  assert_eq!(uris, [None, Some("urn:\u{FFFD}".to_string())]);
}
