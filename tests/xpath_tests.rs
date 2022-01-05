//! xpath module tests
//!

use libxml::parser::Parser;
use libxml::xpath::Context;

#[test]
/// Test the evaluation of an xpath expression yields the correct number of nodes
fn xpath_result_number_correct() {
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let context = Context::new(&doc).unwrap();

  let result1 = context.evaluate("//child").unwrap();
  assert_eq!(result1.get_number_of_nodes(), 2);
  assert_eq!(result1.get_nodes_as_vec().len(), 2);

  let result2 = context.evaluate("//nonexistent").unwrap();
  assert_eq!(result2.get_number_of_nodes(), 0);
  assert_eq!(result2.get_nodes_as_vec().len(), 0);
}

#[test]
/// Test xpath with namespaces
fn xpath_with_namespaces() {
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/simple_namespaces.xml");
  assert!(doc_result.is_ok());

  let doc = doc_result.unwrap();
  let context = Context::new(&doc).unwrap();
  assert!(context
    .register_namespace("h", "http://example.com/ns/hello")
    .is_ok());
  assert!(context
    .register_namespace("f", "http://example.com/ns/farewell")
    .is_ok());
  assert!(context
    .register_namespace("r", "http://example.com/ns/root")
    .is_ok());
  let result_h_td = context.evaluate("//h:td").unwrap();
  assert_eq!(result_h_td.get_number_of_nodes(), 3);
  assert_eq!(result_h_td.get_nodes_as_vec().len(), 3);

  let result_h_table = context.evaluate("//h:table").unwrap();
  assert_eq!(result_h_table.get_number_of_nodes(), 2);
  assert_eq!(result_h_table.get_nodes_as_vec().len(), 2);

  let result_f_footer = context.evaluate("//f:footer").unwrap();
  assert_eq!(result_f_footer.get_number_of_nodes(), 2);
  assert_eq!(result_f_footer.get_nodes_as_vec().len(), 2);

  let result_r = context.evaluate("//r:*").unwrap();
  assert_eq!(result_r.get_number_of_nodes(), 1);
  assert_eq!(result_r.get_nodes_as_vec().len(), 1);

  let result_h = context.evaluate("//h:*").unwrap();
  assert_eq!(result_h.get_number_of_nodes(), 7);
  assert_eq!(result_h.get_nodes_as_vec().len(), 7);

  let result_f = context.evaluate("//f:*").unwrap();
  assert_eq!(result_f.get_number_of_nodes(), 4);
  assert_eq!(result_f.get_nodes_as_vec().len(), 4);

  let result_all = context.evaluate("//*").unwrap();
  assert_eq!(result_all.get_number_of_nodes(), 12);
  assert_eq!(result_all.get_nodes_as_vec().len(), 12);

  let result_h_table = context.evaluate("//table").unwrap();
  assert_eq!(result_h_table.get_number_of_nodes(), 0);
  assert_eq!(result_h_table.get_nodes_as_vec().len(), 0);

  assert!(doc.as_node().recursively_remove_namespaces().is_ok());
  let result_h_table = context.evaluate("//table").unwrap();
  assert_eq!(result_h_table.get_number_of_nodes(), 2);
  assert_eq!(result_h_table.get_nodes_as_vec().len(), 2);
}

#[test]
/// Test that an xpath expression finds the correct node and
/// that the class names are interpreted correctly.
fn class_names() {
  let parser = Parser::default_html();
  let doc_result = parser.parse_file("tests/resources/file02.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let context = Context::new(&doc).unwrap();

  let p_result = context.evaluate("/html/body/p");
  assert!(p_result.is_ok());
  let p = p_result.unwrap();
  assert_eq!(p.get_number_of_nodes(), 1);

  let node = &p.get_nodes_as_vec()[0];
  let names = node.get_class_names();
  assert_eq!(names.len(), 2);
  assert!(names.contains("paragraph"));
  assert!(names.contains("important"));
  assert!(!names.contains("nonsense"));
}

#[test]
/// Test that an xpath string() function processed correctly
fn xpath_string_function() {
  let parser = Parser::default_html();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let context = Context::new(&doc).unwrap();

  let p_result = context.evaluate("string(//root//child[1]/@attribute)");
  assert!(p_result.is_ok());
  let p = p_result.unwrap();
  // Not a node really
  assert_eq!(p.get_number_of_nodes(), 0);
  let content = p.to_string();
  assert_eq!(content, "value");
}

#[test]
/// Test that the dual findnodes interfaces are operational
fn findnodes_interfaces() {
  let parser = Parser::default_html();
  let doc_result = parser.parse_file("tests/resources/file02.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();

  // Xpath interface
  let mut context = Context::new(&doc).unwrap();
  let body = context.evaluate("/html/body").unwrap().get_nodes_as_vec();
  let p_result = context.findnodes("p", body.first());
  assert!(p_result.is_ok());
  let p = p_result.unwrap();
  assert_eq!(p.len(), 1);

  // Node interface
  let body_node = body.first().unwrap();
  let p2_result = body_node.findnodes("p");
  assert!(p2_result.is_ok());
  let p2 = p2_result.unwrap();
  assert_eq!(p2.len(), 1);
}

#[test]
/// Clone is safe on Context objects
fn safe_context_clone() {
  let parser = Parser::default_html();
  let doc_result = parser.parse_file("tests/resources/file02.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();

  // Xpath interface
  let context = Context::new(&doc).unwrap();
  let body = context.evaluate("/html/body").unwrap().get_nodes_as_vec();
  assert_eq!(body.len(), 1);
  let context2 = context.clone();
  let body2 = context2.evaluate("/html/body").unwrap().get_nodes_as_vec();
  assert_eq!(body2.len(), 1);
}

#[test]
fn cleanup_safely_unlinked_xpath_nodes() {
  let p = Parser::default();
  let doc_result = p.parse_string(r##"<?xml version="1.0" standalone="no"?> <!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.0//EN" "http://www.w3.org/TR/2001/REC-SVG-20010904/DTD/svg10.dtd" > <svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
<defs >
<font horiz-adv-x="874" ><font-face
    font-family="Luxi Serif"
    units-per-em="2048"
    panose-1="2 2 7 3 7 0 0 0 0 4"
    ascent="2073"
    descent="-432"
    alphabetic="0" />
<missing-glyph horiz-adv-x="512" d="M51 0V1480H461V0H51ZM410 51V1429H102V51H410Z" />
<glyph unicode=" " glyph-name="space" horiz-adv-x="512" />
<c g1="one" g2="X" k="32" />
</font>
</defs>
</svg>
"##);
  assert!(doc_result.is_ok(), "successfully parsed SVG snippet");
  let doc = doc_result.unwrap();
  let mut xpath = libxml::xpath::Context::new(&doc).unwrap();
  xpath
    .register_namespace("svg", "http://www.w3.org/2000/svg")
    .unwrap();
  for mut k in xpath.findnodes("//svg:c", None).unwrap() {
    k.unlink_node();
  }
  drop(xpath);
  drop(doc);
  assert!(true, "Drops went OK.");
}

/// Tests for the fn xml_xpath_compiles
mod compile_tests {
  use libxml::xpath::xml_xpath_compiles;

  #[test]
  fn can_compile_an_xpath() {
      let compiles = xml_xpath_compiles("//a");
      assert_eq!(compiles, true);
  }

  #[test]
  fn invalid_xpath_does_not_compile() {
      let compiles = xml_xpath_compiles("//a[but invalid]");
      assert_eq!(compiles, false);
  }
}