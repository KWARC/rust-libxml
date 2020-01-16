//!
//! Test Schema Loading, XML Validating
//!
use libxml::schemas::SchemaParserContext;
use libxml::schemas::SchemaValidationContext;

use libxml::parser::Parser;

static SCHEMA: &'static str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:element name="note">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="to" type="xs:string"/>
        <xs:element name="from" type="xs:string"/>
        <xs:element name="heading" type="xs:string"/>
        <xs:element name="body" type="xs:string"/>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
</xs:schema>
"#;

static XML: &'static str = r#"<?xml version="1.0"?>
<note>
  <to>Tove</to>
  <from>Jani</from>
  <heading>Reminder</heading>
  <body>Don't forget me this weekend!</body>
</note>
"#;

static INVALID_XML: &'static str = r#"<?xml version="1.0"?>
<note>
  <bad>Tove</bad>
  <another>Jani</another>
  <heading>Reminder</heading>
  <body>Don't forget me this weekend!</body>
</note>
"#;

#[test]
fn schema_from_string() {
  let xml = Parser::default()
    .parse_string(XML)
    .expect("Expected to be able to parse XML Document from string");

  let mut xsdparser = SchemaParserContext::from_buffer(SCHEMA);
  let xsd = SchemaValidationContext::from_parser(&mut xsdparser);

  if let Err(errors) = xsd {
    for err in &errors {
      println!("{}", err.message());
    }

    panic!("Failed to parse schema");
  }

  let mut xsdvalidator = xsd.unwrap();

  // loop over more than one validation to test for leaks in the error handling callback interactions
  for _ in 0..5 {
    if let Err(errors) = xsdvalidator.validate_document(&xml) {
      for err in &errors {
        println!("{}", err.message());
      }

      panic!("Invalid XML accoding to XSD schema");
    }
  }
}

#[test]
fn schema_from_string_generates_errors() {
  let xml = Parser::default()
    .parse_string(INVALID_XML)
    .expect("Expected to be able to parse XML Document from string");

  let mut xsdparser = SchemaParserContext::from_buffer(SCHEMA);
  let xsd = SchemaValidationContext::from_parser(&mut xsdparser);

  if let Err(errors) = xsd {
    for err in &errors {
      println!("{}", err.message());
    }

    panic!("Failed to parse schema");
  }

  let mut xsdvalidator = xsd.unwrap();
  for _ in 0..5 {
    if let Err(errors) = xsdvalidator.validate_document(&xml) {
      for err in &errors {
        assert_eq!(
          "Element 'bad': This element is not expected. Expected is ( to ).\n",
          err.message()
        );
      }
    }
  }
}
