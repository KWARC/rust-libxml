//! A schema parser context reads its buffer or document only when the schema is parsed: both must live until then.
mod common;

use common::quarantine::{is_freed, setup};
use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};

static NOTE_SCHEMA: &str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:element name="note">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="to" type="xs:string"/>
        <xs:element name="body" type="xs:string"/>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
</xs:schema>
"#;

static VALID_NOTE: &str = "<note><to>Tove</to><body>Hi</body></note>";
static INVALID_NOTE: &str = "<note><body>Hi</body><to>Tove</to></note>";

/// A buffer that overwrites its bytes when dropped, as a reuse of its freed allocation would.
struct Scrubbed(Vec<u8>);

impl AsRef<[u8]> for Scrubbed {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl Drop for Scrubbed {
  fn drop(&mut self) {
    self.0.fill(b'!');
  }
}

#[test]
fn from_buffer_keeps_the_schema_until_it_is_parsed() {
  setup();
  let mut parser = SchemaParserContext::from_buffer(Scrubbed(NOTE_SCHEMA.as_bytes().to_vec()));
  let validator = SchemaValidationContext::from_parser(&mut parser);
  assert!(validator.is_ok(), "{:?}", validator.err());
}

#[test]
fn from_document_keeps_the_document_until_the_schema_is_parsed() {
  setup();
  let doc = Parser::default().parse_string(NOTE_SCHEMA).unwrap();
  let doc_ptr = doc.doc_ptr();
  let mut parser = SchemaParserContext::from_document(&doc);
  drop(doc);
  assert!(!is_freed(doc_ptr), "the parser context reads the document");
  let validator = SchemaValidationContext::from_parser(&mut parser);
  assert!(validator.is_ok(), "{:?}", validator.err());
}

#[test]
fn a_schema_outlives_the_document_it_was_parsed_from() {
  setup();
  let doc = Parser::default().parse_string(NOTE_SCHEMA).unwrap();
  let doc_ptr = doc.doc_ptr();
  let mut validator = {
    let mut parser = SchemaParserContext::from_document(&doc);
    SchemaValidationContext::from_parser(&mut parser).unwrap()
  };
  drop(doc);
  assert!(is_freed(doc_ptr));
  let valid = Parser::default().parse_string(VALID_NOTE).unwrap();
  assert!(validator.validate_document(&valid).is_ok());
  let invalid = Parser::default().parse_string(INVALID_NOTE).unwrap();
  assert!(validator.validate_document(&invalid).is_err());
}

/// Keeping the document leaves the context's auto traits as they were.
#[test]
fn a_schema_parser_context_stays_unwind_safe() {
  fn unwind_safe<T: std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}
  unwind_safe::<SchemaParserContext>();
}
