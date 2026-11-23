//! `Node::for_each_property` stops once `f` frees the node, instead of reading the freed element again. On the system
//! allocator, without the quarantine heap of the other tests, which keeps freed blocks allocated, so that valgrind (CI's
//! valgrind job) reports such a read.
use libxml::parser::Parser;

#[test]
fn for_each_property_stops_once_f_frees_the_node() {
  let doc = Parser::default()
    .parse_string("<r><c a='1' b='2' d='3'/></r>")
    .unwrap();
  let c = doc
    .get_root_element()
    .unwrap()
    .get_first_element_child()
    .unwrap();
  let mut handle = Some(c.clone());
  let mut seen = Vec::new();
  c.for_each_property(|name, _| {
    seen.push(name.to_string());
    if let Some(node) = handle.take() {
      node.free_subtree();
    }
  });
  assert_eq!(seen, ["a"]);
  assert!(c.is_null());
}
