//! A `Namespace` never points into a freed document. On the system allocator, without the quarantine heap, so that
//! valgrind reports the reads these tests would otherwise make of freed memory.
use libxml::parser::Parser;

/// The namespace outlives the `Document` and the `Node` it came from, and reads its document's `xmlNs`.
#[test]
fn a_namespace_reads_its_document_after_its_handles_drop() {
  let ns = {
    let doc = Parser::default()
      .parse_string(r#"<p:r xmlns:p="urn:p"/>"#)
      .unwrap();
    doc.get_root_element().unwrap().get_namespace().unwrap()
  };
  assert_eq!(ns.get_prefix(), "p");
  assert_eq!(ns.get_href(), "urn:p");
}

/// A namespace of a document that then drops, set on a node of another, would be read when that node is serialized.
#[test]
fn a_namespace_of_another_document_never_dangles() {
  let doc = Parser::default().parse_string("<s/>").unwrap();
  let mut s = doc.get_root_element().unwrap();
  {
    let other = Parser::default()
      .parse_string(r#"<r xmlns:p="urn:p"/>"#)
      .unwrap();
    let p = other
      .get_root_element()
      .unwrap()
      .get_namespace_declarations()
      .remove(0);
    let _ = s.set_namespace(&p);
    let _ = s.set_property_ns("a", "1", &p);
  }
  let _ = doc.to_string();
}
