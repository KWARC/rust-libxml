use libxml::parser::Parser;
use rayon::prelude::*;

#[test]
/// Root node and first child of root node are different
/// (There is a tiny chance this might fail for a correct program)
fn child_of_root_has_different_hash() {
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let root = doc.get_root_element().unwrap();
  assert!(!root.is_text_node());

  root.get_child_nodes().into_par_iter().for_each(|mut child| {
    assert!(root != child);
    assert!(child.set_attribute("into_par_iter","true").is_ok());
  });
  let expected = r###"
<?xml version="1.0" encoding="UTF-8"?>
<root>
    <child attribute="value" into_par_iter="true">some text</child>
    <child attribute="empty" into_par_iter="true">more text</child>
</root>
    "###.trim();
    assert_eq!(doc.to_string(true).trim(), expected);
}
