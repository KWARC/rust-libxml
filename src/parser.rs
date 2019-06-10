//! The parser functionality

use crate::bindings::*;
use crate::c_helpers::*;
use crate::tree::*;

use std::convert::AsRef;
use std::ffi::{CStr, CString};
use std::fmt;
use std::ptr;
use std::str;

enum XmlParserOption {
  Recover = 1, // Relaxed parsing
  // XML_PARSE_NODEFDTD = 4, // do not default a doctype if not found
  Noerror = 32, // suppress error reports
  Nowarning = 64, // suppress warning reports
                // XML_PARSE_PEDANTIC = 128, // pedantic error reporting
                // XML_PARSE_NOBLANKS = 256, // remove blank nodes
                // XML_PARSE_NONET = 2048, // Forbid network access
                // XML_PARSE_NOIMPLIED = 8192, // Do not add implied Xml/body... elements
                // XML_PARSE_COMPACT = 65536, // compact small text nodes
                // XML_PARSE_IGNORE_ENC = 2097152, // ignore internal document encoding hint
}

enum HtmlParserOption {
  Recover = 1, // Relaxed parsing
  // HTML_PARSE_NODEFDTD = 4, // do not default a doctype if not found
  Noerror = 32, // suppress error reports
  Nowarning = 64, // suppress warning reports
                // HTML_PARSE_PEDANTIC = 128, // pedantic error reporting
                // HTML_PARSE_NOBLANKS = 256, // remove blank nodes
                // HTML_PARSE_NONET = 2048, // Forbid network access
                // HTML_PARSE_NOIMPLIED = 8192, // Do not add implied html/body... elements
                // HTML_PARSE_COMPACT = 65536, // compact small text nodes
                // HTML_PARSE_IGNORE_ENC = 2097152, // ignore internal document encoding hint
}

///Parser Errors
pub enum XmlParseError {
  ///Parsing returned a null pointer as document pointer
  GotNullPointer,
  ///Document too large for libxml2.
  DocumentTooLarge,
}

impl fmt::Debug for XmlParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      XmlParseError::GotNullPointer => write!(f, "Got a Null pointer"),
      XmlParseError::DocumentTooLarge => write!(f, "Document too large for i32."),
    }
  }
}

/// Default encoding when not provided.
const DEFAULT_ENCODING: *const ::std::os::raw::c_char = ptr::null();

/// Default URL when not provided.
const DEFAULT_URL: *const ::std::os::raw::c_char = ptr::null();

///Convert usize to i32 safely.
fn usize_to_i32(value: usize) -> Result<i32, XmlParseError> {
  if cfg!(target_pointer_width = "16") {
    // Cannot safely use our value comparison, but the conversion if always safe.
      Ok(value as i32)
  } else {
    if value < i32::max_value() as usize {
      // If the value can be safely represented as a 32-bit signed integer.
      Ok(value as i32)
    } else {
      // Document too large, cannot parse using libxml2.
      Err(XmlParseError::DocumentTooLarge)
    }
  }
}

#[derive(PartialEq)]
/// Enum for the parse formats supported by libxml2
pub enum ParseFormat {
  /// Strict parsing for XML
  XML,
  /// Relaxed parsing for HTML
  HTML,
}
/// Parsing API wrapper for libxml2
pub struct Parser {
  /// The `ParseFormat` for this parser
  pub format: ParseFormat,
}
impl Default for Parser {
  /// Create a parser for XML documents
  fn default() -> Self {
    Parser {
      format: ParseFormat::XML,
    }
  }
}
impl Parser {
  /// Create a parser for HTML documents
  pub fn default_html() -> Self {
    Parser {
      format: ParseFormat::HTML,
    }
  }

  /// Parses the XML/HTML file `filename` to generate a new `Document`
  pub fn parse_file(&self, filename: &str) -> Result<Document, XmlParseError> {
    self.parse_file_with_encoding(filename, None)
  }

  /// Parses the XML/HTML file `filename` with a manually-specified encoding
  /// to generate a new `Document`
  pub fn parse_file_with_encoding(&self, filename: &str, encoding: Option<&str>)
    -> Result<Document, XmlParseError>
  {
    // Process filename.
    let filename_cstring = CString::new(filename).unwrap();
    let filename_ptr = filename_cstring.as_ptr();

    // Process encoding.
    let encoding_cstring: Option<CString> = encoding.
      map(|v| CString::new(v).unwrap());
    let encoding_ptr = match encoding_cstring {
      Some(v) => v.as_ptr(),
      None => DEFAULT_ENCODING,
    };

    unsafe {
      xmlKeepBlanksDefault(1);
    }
    match self.format {
      ParseFormat::XML => {
        let options: i32 = XmlParserOption::Recover as i32
          + XmlParserOption::Noerror as i32
          + XmlParserOption::Nowarning as i32;
        unsafe {
          let doc_ptr = xmlReadFile(filename_ptr, encoding_ptr, options);
          if doc_ptr.is_null() {
            Err(XmlParseError::GotNullPointer)
          } else {
            Ok(Document::new_ptr(doc_ptr))
          }
        }
      }
      ParseFormat::HTML => {
        // TODO: Allow user-specified options later on
        let options: i32 = HtmlParserOption::Recover as i32
          + HtmlParserOption::Noerror as i32
          + HtmlParserOption::Nowarning as i32;
        unsafe {
          let doc_ptr = htmlReadFile(filename_ptr, encoding_ptr, options);
          if doc_ptr.is_null() {
            Err(XmlParseError::GotNullPointer)
          } else {
            Ok(Document::new_ptr(doc_ptr))
          }
        }
      }
    }
  }

  ///Parses the XML/HTML bytes `input` to generate a new `Document`
  pub fn parse_string<Bytes: AsRef<[u8]>>(&self, input: Bytes) -> Result<Document, XmlParseError> {
    self.parse_string_with_encoding(input, None)
  }

  ///Parses the XML/HTML bytes `input` with a manually-specified
  ///encoding to generate a new `Document`
  pub fn parse_string_with_encoding<Bytes: AsRef<[u8]>>(&self, input: Bytes, encoding: Option<&str>)
    -> Result<Document, XmlParseError>
  {
    // Process input bytes.
    let input_bytes = input.as_ref();
    let input_ptr = input_bytes.as_ptr() as *const ::std::os::raw::c_char;
    let input_len = usize_to_i32(input_bytes.len())?;

    // Process encoding.
    let encoding_cstring: Option<CString> = encoding.
      map(|v| CString::new(v).unwrap());
    let encoding_ptr = match encoding_cstring {
      Some(v) => v.as_ptr(),
      None => DEFAULT_ENCODING,
    };

    // Process url.
    let url_ptr = DEFAULT_URL;
    match self.format {
      ParseFormat::XML => unsafe {
        let options: i32 = XmlParserOption::Recover as i32
          + XmlParserOption::Noerror as i32
          + XmlParserOption::Nowarning as i32;
        let docptr = xmlReadMemory(
          input_ptr,
          input_len,
          url_ptr,
          encoding_ptr,
          options,
        );
        if docptr.is_null() {
          Err(XmlParseError::GotNullPointer)
        } else {
          Ok(Document::new_ptr(docptr))
        }
      },
      ParseFormat::HTML => unsafe {
        let options: i32 = HtmlParserOption::Recover as i32
          + HtmlParserOption::Noerror as i32
          + HtmlParserOption::Nowarning as i32;
        let docptr = htmlReadMemory(
          input_ptr,
          input_len,
          url_ptr,
          encoding_ptr,
          options,
        );
        if docptr.is_null() {
          Err(XmlParseError::GotNullPointer)
        } else {
          Ok(Document::new_ptr(docptr))
        }
      },
    }
  }

  /// Checks a string for well-formedness.
  pub fn is_well_formed_html<Bytes: AsRef<[u8]>>(&self, input: Bytes) -> bool {
    self.is_well_formed_html_with_encoding(input, None)
  }

  /// Checks a string for well-formedness with manually-specified encoding.
  /// IMPORTANT: This function is currently implemented in a HACKY way, to ignore invalid errors for HTML5 elements (such as <math>)
  ///            this means you should NEVER USE IT WHILE THREADING, it is CERTAIN TO BREAK
  ///
  /// Help is welcome in implementing it correctly.
  pub fn is_well_formed_html_with_encoding<Bytes: AsRef<[u8]>>(&self, input: Bytes, encoding: Option<&str>)
    -> bool
  {
    // Process input string.
    let input_bytes = input.as_ref();
    if input_bytes.is_empty() {
      return false;
    }
    let input_ptr = input_bytes.as_ptr() as *const ::std::os::raw::c_char;
    let input_len = match usize_to_i32(input_bytes.len()) {
      Ok(v) => v,
      Err(_) => return false,
    };

    // Process encoding.
    let encoding_cstring: Option<CString> = encoding.
      map(|v| CString::new(v).unwrap());
    let encoding_ptr = match encoding_cstring {
      Some(v) => v.as_ptr(),
      None => DEFAULT_ENCODING,
    };

    // Process url.
    let url_ptr = DEFAULT_URL;
    // disable generic error lines from libxml2
    match self.format {
      ParseFormat::XML => false, // TODO: Add support for XML at some point
      ParseFormat::HTML => unsafe {
        let ctxt = htmlNewParserCtxt();
        setWellFormednessHandler(ctxt);
        let docptr = htmlCtxtReadMemory (
          ctxt,
          input_ptr,
          input_len,
          url_ptr,
          encoding_ptr,
          10_596,
        ); // htmlParserOption = 4+32+64+256+2048+8192
        let well_formed_final = if htmlWellFormed(ctxt) {
          // Basic well-formedness passes, let's check if we have an <html> element as root too
          if !docptr.is_null() {
            let node_ptr = xmlDocGetRootElement(docptr);
            let name_ptr = xmlNodeGetName(node_ptr);
            if name_ptr.is_null() {
              false
            }
            //empty string
            else {
              let c_root_name = CStr::from_ptr(name_ptr);
              let root_name = str::from_utf8(c_root_name.to_bytes()).unwrap().to_owned();
              root_name == "html"
            }
          } else {
            false
          }
        } else {
          false
        };

        if !ctxt.is_null() {
          htmlFreeParserCtxt(ctxt);
        }
        if !docptr.is_null() {
          xmlFreeDoc(docptr);
        }
        well_formed_final
      },
    }
  }
}
