//! A few random tests.
//! Knowing how much I neglect this aspect of software development,
//! there probably won't be a significant coverage.

extern crate libxml;

use std::fs::File;
use std::io::Read;

use libxml::tree::{Document, Node, Namespace, NodeType};
use libxml::xpath::Context;
use libxml::parser::Parser;

#[test]
/// Build a hello world XML doc
fn hello_builder() {
  let doc_result = Document::new();
  assert!(doc_result.is_ok());
  let mut doc = doc_result.unwrap();

  let doc_node = doc.get_root_element();
  assert_eq!(doc_node.get_type(), Some(NodeType::DocumentNode));

  let hello_element_result = Node::new("hello", None, &doc);
  assert!(hello_element_result.is_ok());
  let mut hello_element = hello_element_result.unwrap();

  doc.set_root_element(&mut hello_element);

  hello_element.set_content("world!");

  let added = hello_element.new_child(None, "child");
  assert!(added.is_ok());
  let mut new_child = added.unwrap();

  new_child.set_content("set content");

  assert_eq!(new_child.get_content(), "set content");
  assert_eq!(hello_element.get_content(), "world!set content");

  let node_string = doc.node_to_string(&hello_element);
  assert!(node_string.len() > 1);

  let doc_string = doc.to_string(false);
  assert!(doc_string.len() > 1);
  assert!(doc.save_file("tests/results/helloworld.xml").is_ok());
}

#[test]
fn create_pi() {
  let doc_result = Document::new();
  assert!(doc_result.is_ok());
  let mut doc = doc_result.unwrap();
  // Add a PI
  let node_ok = doc.create_processing_instruction("piname", "picontent");
  assert!(node_ok.is_ok());
  let doc_string = doc.to_string(false);
  assert!(doc_string.len() > 1);
}


#[test]
/// Duplicate an xml file
fn duplicate_file() {
  let parser = Parser::default();
  {
    let doc_result = parser.parse_file("tests/resources/file01.xml");
    assert!(doc_result.is_ok());

    let doc = doc_result.unwrap();
    doc.save_file("tests/results/copy.xml").unwrap();
  }
}

#[test]
// Can parse an xml string in memory
fn can_parse_xml_string() {
  let mut file = File::open("tests/resources/file01.xml").unwrap();
  let mut xml_string = String::new();
  file.read_to_string(&mut xml_string).unwrap();
  let parser = Parser::default();
  let doc = parser.parse_string(&xml_string).unwrap();
  assert_eq!(doc.get_root_element().get_name(), "root");
}

#[test]
/// Can load an HTML file
fn can_load_html_file() {
  let parser = Parser::default_html();
  {
    let doc_result = parser.parse_file("tests/resources/example.html");
    assert!(doc_result.is_ok());

    let doc = doc_result.unwrap();
    let root = doc.get_root_element();
    assert_eq!(root.get_name(), "html");
  }
}

#[test]
/// Root node and first child of root node are different
/// (There is a tiny chance this might fail for a correct program)
fn child_of_root_has_different_hash() {
  let parser = Parser::default();
  {
    let doc_result = parser.parse_file("tests/resources/file01.xml");
    assert!(doc_result.is_ok());
    let doc = doc_result.unwrap();
    let root = doc.get_root_element();
    assert!(!root.is_text_node());
    if let Some(child) = root.get_first_child() {
      assert!(root != child);
    } else {
      assert!(false);   //test failed - child doesn't exist
    }
    // same check with last child
    if let Some(child) = root.get_last_child() {
      assert!(root != child);
    } else {
      assert!(false);   //test failed - child doesn't exist
    }
  }
}

#[test]
/// Siblings basic unit tests
fn node_sibling_accessors() {
  let mut doc = Document::new().unwrap();
  let hello_element_result = Node::new("hello", None, &doc);
  assert!(hello_element_result.is_ok());
  let mut hello_element = hello_element_result.unwrap();
  doc.set_root_element(&mut hello_element);

  let new_sibling = Node::new("sibling", None, &doc).unwrap();
  assert!(hello_element.add_prev_sibling(new_sibling).is_some());
}

#[test]
fn node_children_accessors() {
  // Setup
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let root = doc.get_root_element();

  // Tests
  let root_children = root.get_child_nodes();
  assert_eq!(root_children.len(), 5, "file01 root has five child nodes");
  let mut element_children = root.get_child_elements();
  assert_eq!(element_children.len(), 2, "file01 root has two child elements");
  assert_eq!(element_children.pop().unwrap().get_name(), "child");
  assert_eq!(element_children.pop().unwrap().get_name(), "child");
  assert!(element_children.is_empty());
}

#[test]
fn node_attributes_accessor() {
  // Setup
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());
  let doc = doc_result.unwrap();
  let root = doc.get_root_element();
  let mut root_elements = root.get_child_elements();
  let child_opt = root_elements.first_mut();
  assert!(child_opt.is_some());
  let mut child = child_opt.unwrap();

  // All attributes
  let attributes = child.get_attributes();
  assert_eq!(attributes.len(), 1);
  assert_eq!(attributes.get("attribute"), Some(&"value".to_string()));

  // Get
  assert_eq!(child.get_attribute("attribute"), Some("value".to_string()));
  // Get as node
  let attr_node_opt = child.get_attribute_node("attribute");
  assert!(attr_node_opt.is_some());
  let attr_node = attr_node_opt.unwrap();
  assert_eq!(attr_node.get_name(), "attribute");
  assert_eq!(attr_node.get_type(), Some(NodeType::AttributeNode));

  // Set
  child.set_attribute("attribute","setter_value");
  assert_eq!(child.get_attribute("attribute"), Some("setter_value".to_string()));
  // Remove
  child.remove_attribute("attribute");
  assert_eq!(child.get_attribute("attribute"), None);
  // Recount
  let attributes = child.get_attributes();
  assert_eq!(attributes.len(), 0);
}

#[test]
fn attribute_namespace_accessors() {
  let mut doc = Document::new().unwrap();
  let element_result = Node::new("example", None, &doc);
  assert!(element_result.is_ok());

  let mut element = element_result.unwrap();
  doc.set_root_element(&element);

  let ns_result = Namespace::new("myxml", "http://www.w3.org/XML/1998/namespace", &element);
  assert!(ns_result.is_ok());
  let ns = ns_result.unwrap();
  element.set_attribute_ns("id", "testing", ns);

  let id_attr = element.get_attribute_ns("id", "http://www.w3.org/XML/1998/namespace");
  assert!(id_attr.is_some());
  assert_eq!(id_attr.unwrap(), "testing");

  let id_regular = element.get_attribute("id");
  assert!(id_regular.is_some());
  assert_eq!(id_regular.unwrap(), "testing");

  let id_false_ns = element.get_attribute_ns("id", "http://www.foobar.org");
  assert!(id_false_ns.is_none());
  let fb_ns_result = Namespace::new("fb", "http://www.foobar.org", &element);
  assert!(fb_ns_result.is_ok());
  let fb_ns = fb_ns_result.unwrap();
  element.set_attribute_ns("fb", "fb", fb_ns);

  let ns_prefix = element.lookup_namespace_prefix("http://www.w3.org/XML/1998/namespace");
  assert_eq!(ns_prefix, Some("xml".to_string())); // system ns has the global prefix when doing global lookup
  let fb_prefix = element.lookup_namespace_prefix("http://www.foobar.org");
  assert_eq!(fb_prefix, Some("fb".to_string())); // system ns has the global prefix when doing global lookup

  let ns_uri = element.lookup_namespace_uri("myxml");
  assert_eq!(ns_uri, Some("http://www.w3.org/XML/1998/namespace".to_string())); // system ns has the global uri when doing global lookup
  let fb_uri = element.lookup_namespace_uri("fb");
  assert_eq!(fb_uri, Some("http://www.foobar.org".to_string())); // system ns has the global prefix when doing global lookup
}

#[test]
fn node_can_unbind() {
  let mut doc = Document::new().unwrap();
  let element_result = Node::new("example", None, &doc);
  assert!(element_result.is_ok());

  let mut element = element_result.unwrap();
  doc.set_root_element(&element);

  let first = Node::new("first", None, &doc).unwrap();
  let second = Node::new("second", None, &doc).unwrap();
  let third = Node::new("third", None, &doc).unwrap();

  let mut first_child = element.add_child(first).unwrap();
  let mut second_child = element.add_child(second).unwrap();
  let mut third_child = element.add_child(third).unwrap();

  assert_eq!(element.get_child_nodes().len(), 3);
  first_child.unbind_node();
  assert_eq!(element.get_child_nodes().len(), 2);
  second_child.unlink_node();
  assert_eq!(element.get_child_nodes().len(), 1);
  third_child.unlink();
  assert_eq!(element.get_child_nodes().len(), 0);

  // Test reparenting via unlink
  let transfer = Node::new("transfer", None, &doc).unwrap();
  let mut transfer_child = element.add_child(transfer).unwrap();
  transfer_child.append_text("test text");
  let receiver = Node::new("receiver", None, &doc).unwrap();
  let mut receiver_child = element.add_child(receiver).unwrap();
  assert_eq!(element.get_child_nodes().len(), 2);
  assert_eq!(transfer_child.get_child_nodes().len(), 1);
  assert_eq!(receiver_child.get_child_nodes().len(), 0);

  transfer_child.unlink();
  assert_eq!(element.get_child_nodes().len(), 1);
  assert_eq!(receiver_child.get_child_nodes().len(), 0);
  let reparented_transfer = receiver_child.add_child(transfer_child).unwrap();
  assert_eq!(receiver_child.get_child_nodes().len(), 1);
  assert_eq!(reparented_transfer.get_content(), "test text".to_owned());
}

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
  assert!(context.register_namespace("h", "http://example.com/ns/hello").is_ok());
  assert!(context.register_namespace("f", "http://example.com/ns/farewell").is_ok());
  assert!(context.register_namespace("r", "http://example.com/ns/root").is_ok());
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
/// Test well-formedness of a Rust string
/// IMPORTANT: Currenlty NOT THREAD-SAFE, use in single-threaded apps only!
fn well_formed_html() {
  let parser = Parser::default_html();

  let trivial_well_formed = parser.is_well_formed_html("<!DOCTYPE html>\n<html><head></head><body></body></html>");
  assert!(trivial_well_formed);

  let trivial_ill_formed = parser.is_well_formed_html("garbage");
  assert!(!trivial_ill_formed);

  let should_ill_formed = parser.is_well_formed_html("<broken <markup>> </boom>");
  assert!(!should_ill_formed);

  let should_well_formed = parser.is_well_formed_html("<!DOCTYPE html>\n<html><head><title>Test</title></head><body>\n<h1>Tiny</h1><math><mn>2</mn></math></body></html>");
  assert!(should_well_formed);
}

#[test]
/// Can mock a node object (useful for defaults that will be overridden)
fn can_mock_node() {
  let node_mock = Node::mock();
  assert!(!node_mock.is_text_node());
}

#[test]
/// Can make a mock node hashable
fn can_hash_mock_node() {
  let node_mock = Node::mock();
  assert!(node_mock.to_hashable() > 0);
}

#[test]
/// Can set and get attributes
fn can_manage_attributes() {
  let mut doc = Document::new().unwrap();
  let hello_element_result = Node::new("hello", None, &doc);
  assert!(hello_element_result.is_ok());
  let mut hello_element = hello_element_result.unwrap();
  doc.set_root_element(&mut hello_element);

  let key = "examplekey";
  let value = "examplevalue";
  let pre_value = hello_element.get_attribute(key);
  assert_eq!(pre_value, None);
  let pre_prop_value = hello_element.get_property(key);
  assert_eq!(pre_prop_value, None);

  hello_element.set_attribute(key, value);
  let new_value = hello_element.get_attribute(key);
  assert_eq!(new_value, Some(value.to_owned()));
}

#[test]
/// Can set and get text node content
fn can_set_get_text_node_content() {
  let mut doc = Document::new().unwrap();
  let hello_element_result = Node::new("hello", None, &doc);
  assert!(hello_element_result.is_ok());
  let mut hello_element = hello_element_result.unwrap();
  doc.set_root_element(&mut hello_element);

  assert!( hello_element.get_content().is_empty() );
  hello_element.append_text("hello ");
  assert_eq!(hello_element.get_content(), "hello ");
  hello_element.append_text("world!");
  assert_eq!(hello_element.get_content(), "hello world!");

}

#[test]
/// Basic namespace workflow
fn can_work_with_namespaces() {
  let mut doc = Document::new().unwrap();
  let mut root_node = Node::new("root", None, &doc).unwrap();
  doc.set_root_element(&root_node);

  let initial_namespace_list = root_node.get_namespaces(&doc);
  assert_eq!(initial_namespace_list.len(), 0);

  let mock_ns_result = Namespace::new("mock", "http://example.com/ns/mock", &root_node);
  assert!(mock_ns_result.is_ok());
  let second_ns_result = Namespace::new("second", "http://example.com/ns/second", &root_node);
  assert!(second_ns_result.is_ok());

  // try to attach this namespace to a node
  assert!(root_node.get_namespace().is_none());
  root_node.set_namespace(mock_ns_result.unwrap());
  let active_ns_opt = root_node.get_namespace();
  assert!(active_ns_opt.is_some());
  let active_ns = active_ns_opt.unwrap();
  assert_eq!(active_ns.get_prefix(), "mock");
  assert_eq!(active_ns.get_href(), "http://example.com/ns/mock");

  // now get all namespaces for the node and check we have ours
  let mut namespace_list = root_node.get_namespaces(&doc);
  assert_eq!(namespace_list.len(), 2);

  let second_ns = namespace_list.pop().unwrap();
  assert_eq!(second_ns.get_prefix(), "second");
  assert_eq!(second_ns.get_href(), "http://example.com/ns/second");

  let first_ns = namespace_list.pop().unwrap();
  assert_eq!(first_ns.get_prefix(), "mock");
  assert_eq!(first_ns.get_href(), "http://example.com/ns/mock");

  let declarations = root_node.get_namespace_declarations();
  assert_eq!(declarations.len(), 2);
}

#[test]
/// Can view documents as nodes
fn can_cast_doc_to_node() {
  // Setup
  let parser = Parser::default();
  let doc_result = parser.parse_file("tests/resources/file01.xml");
  assert!(doc_result.is_ok());

  let doc = doc_result.unwrap();
  let doc_node = doc.as_node();
  assert_eq!(doc_node.get_type(), Some(NodeType::DocumentNode));
  let root_node_opt = doc_node.get_first_child();
  assert!(root_node_opt.is_some());
  let root_node = root_node_opt.unwrap();
  assert_eq!(root_node.get_name(), "root");
}
