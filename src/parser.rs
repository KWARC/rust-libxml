//! The parser functionality

use crate::bindings::*;
use crate::c_helpers::*;
use crate::tree::*;

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
}

impl fmt::Debug for XmlParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      XmlParseError::GotNullPointer => write!(f, "Got a Null pointer"),
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

  ///Parses the XML/HTML file `filename` to generate a new `Document`
  pub fn parse_file(&self, filename: &str) -> Result<Document, XmlParseError> {
    let c_filename = CString::new(filename).unwrap();
    let c_utf8 = CString::new("utf-8").unwrap();
    let options: i32 = XmlParserOption::Recover as i32
      + XmlParserOption::Noerror as i32
      + XmlParserOption::Nowarning as i32;
    match self.format {
      ParseFormat::XML => unsafe {
        xmlKeepBlanksDefault(1);
        let docptr = xmlReadFile(c_filename.as_ptr(), c_utf8.as_ptr(), options);
        if docptr.is_null() {
          Err(XmlParseError::GotNullPointer)
        } else {
          Ok(Document::new_ptr(docptr))
        }
      },
      ParseFormat::HTML => {
        // TODO: Allow user-specified options later on
        unsafe {
          let options: i32 = HtmlParserOption::Recover as i32
            + HtmlParserOption::Noerror as i32
            + HtmlParserOption::Nowarning as i32;
          xmlKeepBlanksDefault(1);
          let docptr = htmlReadFile(c_filename.as_ptr(), c_utf8.as_ptr(), options);
          if docptr.is_null() {
            Err(XmlParseError::GotNullPointer)
          } else {
            Ok(Document::new_ptr(docptr))
          }
        }
      }
    }
  }

  ///Parses the XML/HTML string `input_string` to generate a new `Document`
  pub fn parse_string(&self, input_string: &str) -> Result<Document, XmlParseError> {
    let c_string = CString::new(input_string).unwrap();
    let c_utf8 = CString::new("utf-8").unwrap();
    let c_url = CString::new("").unwrap();
    match self.format {
      ParseFormat::XML => unsafe {
        let options: i32 = XmlParserOption::Recover as i32
          + XmlParserOption::Noerror as i32
          + XmlParserOption::Nowarning as i32;
        let docptr = xmlReadDoc(
          c_string.as_bytes().as_ptr(),
          c_url.as_ptr(),
          c_utf8.as_ptr(),
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
        let docptr = htmlReadDoc(
          c_string.as_bytes().as_ptr(),
          c_url.as_ptr(),
          c_utf8.as_ptr(),
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

  /// Checks a string for well-formedness
  /// IMPORTANT: This function is currently implemented in a HACKY way, to ignore invalid errors for HTML5 elements (such as <math>)
  ///            this means you should NEVER USE IT WHILE THREADING, it is CERTAIN TO BREAK
  ///
  /// Help is welcome in implementing it correctly.
  pub fn is_well_formed_html(&self, input_string: &str) -> bool {
    if input_string.is_empty() {
      return false;
    }
    let c_string = CString::new(input_string).unwrap();
    let c_utf8 = CString::new("utf-8").unwrap();
    // disable generic error lines from libxml2
    match self.format {
      ParseFormat::XML => false, // TODO: Add support for XML at some point
      ParseFormat::HTML => unsafe {
        let ctxt = htmlNewParserCtxt();
        setWellFormednessHandler(ctxt);
        let docptr = htmlCtxtReadDoc(
          ctxt,
          c_string.as_bytes().as_ptr(),
          ptr::null_mut(),
          c_utf8.as_ptr(),
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
