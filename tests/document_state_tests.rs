//! `Document::is_well_formed` / `is_ns_valid`: libxml2's `xmlDoc.properties` parse flags.
use libxml::parser::Parser;
use libxml::tree::Document;

#[test]
fn well_formed_parse_sets_both_flags() {
  let doc = Parser::default()
    .parse_string("<a xmlns:p='urn:p'><p:b/>x</a>")
    .unwrap();
  assert!(doc.is_well_formed());
  assert!(doc.is_ns_valid());
}

#[test]
fn recovered_parse_is_not_well_formed() {
  // The default parser recovers: the tree exists, the flag says it was repaired.
  let doc = Parser::default()
    .parse_string("<a><b>unclosed</a>")
    .unwrap();
  assert!(!doc.is_well_formed());
  let doc = Parser::default().parse_string("<a>x\u{1}y</a>").unwrap();
  assert!(!doc.is_well_formed());
}

#[test]
fn undeclared_prefix_is_well_formed_but_not_ns_valid() {
  let doc = Parser::default().parse_string("<a><p:b/></a>").unwrap();
  assert!(doc.is_well_formed());
  assert!(!doc.is_ns_valid());
}

#[test]
fn built_document_reports_neither() {
  let doc = Document::new().unwrap();
  assert!(!doc.is_well_formed());
  assert!(!doc.is_ns_valid());
}

#[test]
fn parse_file_sets_both_flags() {
  let doc = Parser::default()
    .parse_file("tests/resources/file01.xml")
    .unwrap();
  assert!(doc.is_well_formed());
  assert!(doc.is_ns_valid());
}

/// libxml2's HTML parser sets neither flag, even on a clean parse.
#[test]
fn html_document_reports_neither() {
  let doc = Parser::default_html()
    .parse_string("<!DOCTYPE html><html><head><title>t</title></head><body><p>x</p></body></html>")
    .unwrap();
  assert!(!doc.is_well_formed());
  assert!(!doc.is_ns_valid());
}

/// `xmlCopyDoc` does not copy `xmlDoc.properties`: every copy of a well-formed parse reports neither flag.
#[test]
fn copies_report_neither() {
  let doc = Parser::default().parse_string("<a><b/></a>").unwrap();
  assert!(doc.is_well_formed() && doc.is_ns_valid());
  let dup = doc.dup().unwrap();
  // (`dup_from` copies into a Document holding no xmlDoc)
  let mut dup_into = Document::new_ptr(std::ptr::null_mut());
  dup_into.dup_from(&doc).unwrap();
  let root = doc.get_root_element().unwrap();
  let node_copy = Document::dup_node_into_new_doc(&root).unwrap();
  for copy in [&dup, &dup_into, &node_copy] {
    assert_eq!(
      copy.node_to_string(&copy.get_root_element().unwrap()),
      "<a><b/></a>"
    );
    assert!(!copy.is_well_formed());
    assert!(!copy.is_ns_valid());
  }
}

/// The flags describe the parse, not the tree: an edit after which a reparse would not be namespace-valid (an
/// undeclared prefix) clears neither.
#[test]
fn edit_after_parse_keeps_the_flags() {
  let doc = Parser::default().parse_string("<a>x</a>").unwrap();
  let mut root = doc.get_root_element().unwrap();
  root.set_name("p:a").unwrap();
  let serialized = doc.to_string();
  assert!(serialized.contains("<p:a>x</p:a>"), "{serialized}");
  assert!(
    !Parser::default()
      .parse_string(&serialized)
      .unwrap()
      .is_ns_valid()
  );
  assert!(doc.is_well_formed());
  assert!(doc.is_ns_valid());
}
