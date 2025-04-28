//! The parser functionality

use crate::bindings::*;
use crate::c_helpers::*;
use crate::tree::*;

use std::convert::AsRef;
use std::error::Error;
use std::ffi::c_void;
use std::ffi::{CStr, CString};
use std::fmt;
use std::fs;
use std::io;
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::slice;
use std::str;
use std::sync::Once;

static INIT_LIBXML_PARSER: Once = Once::new();

enum XmlParserOption {
  Recover = 1,
  Nodefdtd = 4,
  Noerror = 32,
  Nowarning = 64,
  Pedantic = 128,
  Noblanks = 256,
  Nonet = 2048,
  Noimplied = 8192,
  Compact = 65_536,
  Huge = 524_288,
  Ignoreenc = 2_097_152,
}

enum HtmlParserOption {
  Recover = 1,
  Nodefdtd = 4,
  Noerror = 32,
  Nowarning = 64,
  Pedantic = 128,
  Noblanks = 256,
  Nonet = 2048,
  Noimplied = 8192,
  Huge = 524_288,
  Compact = 65_536,
  Ignoreenc = 2_097_152,
}

/// Parser Options
pub struct ParserOptions<'a> {
  /// Relaxed parsing
  pub recover: bool,
  /// do not default a doctype if not found
  pub no_def_dtd: bool,
  /// do not default a doctype if not found
  pub no_error: bool,
  /// suppress warning reports
  pub no_warning: bool,
  /// pedantic error reporting
  pub pedantic: bool,
  /// remove blank nodes
  pub no_blanks: bool,
  /// Forbid network access
  pub no_net: bool,
  /// Do not add implied html/body... elements
  pub no_implied: bool,
  /// relax any hardcoded limit from the parser
  pub huge: bool,
  /// compact small text nodes
  pub compact: bool,
  /// ignore internal document encoding hint
  pub ignore_enc: bool,
  /// manually-specified encoding
  pub encoding: Option<&'a str>,
}

impl ParserOptions<'_> {
  pub(crate) fn to_flags(&self, format: &ParseFormat) -> i32 {
    macro_rules! to_option_flag {
      (
        $condition:expr => $variant:ident
      ) => {
        if $condition {
          match format {
            ParseFormat::HTML => HtmlParserOption::$variant as i32,
            ParseFormat::XML => XmlParserOption::$variant as i32,
          }
        } else {
          0
        }
      };
    }
    // return the combined flags
    to_option_flag!(self.recover => Recover)
      + to_option_flag!(self.no_def_dtd => Nodefdtd)
      + to_option_flag!(self.no_error => Noerror)
      + to_option_flag!(self.no_warning => Nowarning)
      + to_option_flag!(self.pedantic => Pedantic)
      + to_option_flag!(self.no_blanks => Noblanks)
      + to_option_flag!(self.no_net => Nonet)
      + to_option_flag!(self.no_implied => Noimplied)
      + to_option_flag!(self.huge => Huge)
      + to_option_flag!(self.compact => Compact)
      + to_option_flag!(self.ignore_enc => Ignoreenc)
  }
}

impl Default for ParserOptions<'_> {
  fn default() -> Self {
    ParserOptions {
      recover: true,
      no_def_dtd: false,
      no_error: true,
      no_warning: true,
      pedantic: false,
      no_blanks: false,
      no_net: false,
      no_implied: false,
      huge: false,
      compact: false,
      ignore_enc: false,
      encoding: None,
    }
  }
}

///Parser Errors
pub enum XmlParseError {
  ///Parsing returned a null pointer as document pointer
  GotNullPointer,
  ///Could not open file error.
  FileOpenError,
  ///Document too large for libxml2.
  DocumentTooLarge,
}

impl Error for XmlParseError {}

impl fmt::Debug for XmlParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{self}")
  }
}

impl fmt::Display for XmlParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}",
      match self {
        XmlParseError::GotNullPointer => "Got a Null pointer",
        XmlParseError::FileOpenError => "Unable to open path to file.",
        XmlParseError::DocumentTooLarge => "Document too large for i32.",
      }
    )
  }
}

/// Default encoding when not provided.
const DEFAULT_ENCODING: *const c_char = ptr::null();

/// Default URL when not provided.
const DEFAULT_URL: *const c_char = ptr::null();

/// Open file function.
fn xml_open(filename: &str) -> io::Result<*mut c_void> {
  let ptr = Box::into_raw(Box::new(fs::File::open(filename)?));
  Ok(ptr as *mut c_void)
}

/// Read callback for an FS file.
unsafe extern "C" fn xml_read(context: *mut c_void, buffer: *mut c_char, len: c_int) -> c_int {
  // Len is always positive, typically 40-4000 bytes.
  let file = context as *mut fs::File;
  let buf = slice::from_raw_parts_mut(buffer as *mut u8, len as usize);
  match io::Read::read(&mut *file, buf) {
    Ok(v) => v as c_int,
    Err(_) => -1,
  }
}

type XmlReadCallback = unsafe extern "C" fn(*mut c_void, *mut c_char, c_int) -> c_int;

/// Close callback for an FS file.
unsafe extern "C" fn xml_close(context: *mut c_void) -> c_int {
  // Take rust ownership of the context and then drop it.
  let file = context as *mut fs::File;
  let _ = Box::from_raw(file);
  0
}

type XmlCloseCallback = unsafe extern "C" fn(*mut c_void) -> c_int;

///Convert usize to i32 safely.
fn try_usize_to_i32(value: usize) -> Result<i32, XmlParseError> {
  if cfg!(target_pointer_width = "16") || (value < i32::MAX as usize) {
    // Cannot safely use our value comparison, but the conversion if always safe.
    // Or, if the value can be safely represented as a 32-bit signed integer.
    Ok(value as i32)
  } else {
    // Document too large, cannot parse using libxml2.
    Err(XmlParseError::DocumentTooLarge)
  }
}

#[derive(Debug, PartialEq, Eq)]
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
    // avoid deadlocks from using multiple parsers
    INIT_LIBXML_PARSER.call_once(|| unsafe {
      crate::bindings::xmlInitParser();
    });
    Parser {
      format: ParseFormat::XML,
    }
  }
}
impl Parser {
  /// Create a parser for HTML documents
  pub fn default_html() -> Self {
    // avoid deadlocks from using multiple parsers
    INIT_LIBXML_PARSER.call_once(|| unsafe {
      crate::bindings::xmlInitParser();
    });
    Parser {
      format: ParseFormat::HTML,
    }
  }

  /// Parses the XML/HTML file `filename` to generate a new `Document`
  pub fn parse_file(&self, filename: &str) -> Result<Document, XmlParseError> {
    self.parse_file_with_options(filename, ParserOptions::default())
  }

  /// Parses the XML/HTML file `filename` with a manually-specified parser-options
  /// to generate a new `Document`
  pub fn parse_file_with_options(
    &self,
    filename: &str,
    parser_options: ParserOptions,
  ) -> Result<Document, XmlParseError> {
    // Create extern C callbacks for to read and close a Rust file through
    // a void pointer.
    let ioread: Option<XmlReadCallback> = Some(xml_read);
    let ioclose: Option<XmlCloseCallback> = Some(xml_close);
    let ioctx = match xml_open(filename) {
      Ok(v) => v,
      Err(_) => return Err(XmlParseError::FileOpenError),
    };

    // Process encoding.
    let encoding_cstring: Option<CString> =
      parser_options.encoding.map(|v| CString::new(v).unwrap());
    let encoding_ptr = match encoding_cstring {
      Some(v) => v.as_ptr(),
      None => DEFAULT_ENCODING,
    };

    // Process url.
    let url_ptr = DEFAULT_URL;

    unsafe {
      xmlKeepBlanksDefault(1);
    }

    let options = parser_options.to_flags(&self.format);

    match self.format {
      ParseFormat::XML => unsafe {
        let doc_ptr = xmlReadIO(ioread, ioclose, ioctx, url_ptr, encoding_ptr, options);
        if doc_ptr.is_null() {
          Err(XmlParseError::GotNullPointer)
        } else {
          Ok(Document::new_ptr(doc_ptr))
        }
      },
      ParseFormat::HTML => unsafe {
        let doc_ptr = htmlReadIO(ioread, ioclose, ioctx, url_ptr, encoding_ptr, options);
        if doc_ptr.is_null() {
          Err(XmlParseError::GotNullPointer)
        } else {
          Ok(Document::new_ptr(doc_ptr))
        }
      },
    }
  }

  ///Parses the XML/HTML bytes `input` to generate a new `Document`
  pub fn parse_string<Bytes: AsRef<[u8]>>(&self, input: Bytes) -> Result<Document, XmlParseError> {
    self.parse_string_with_options(input, ParserOptions::default())
  }

  ///Parses the XML/HTML bytes `input` with a manually-specified
  ///parser-options to generate a new `Document`
  pub fn parse_string_with_options<Bytes: AsRef<[u8]>>(
    &self,
    input: Bytes,
    parser_options: ParserOptions,
  ) -> Result<Document, XmlParseError> {
    // Process input bytes.
    let input_bytes = input.as_ref();
    let input_ptr = input_bytes.as_ptr() as *const c_char;
    let input_len = try_usize_to_i32(input_bytes.len())?;

    // Process encoding.
    let encoding_cstring: Option<CString> =
      parser_options.encoding.map(|v| CString::new(v).unwrap());
    let encoding_ptr = match encoding_cstring {
      Some(v) => v.as_ptr(),
      None => DEFAULT_ENCODING,
    };

    // Process url.
    let url_ptr = DEFAULT_URL;

    let options = parser_options.to_flags(&self.format);

    match self.format {
      ParseFormat::XML => unsafe {
        let docptr = xmlReadMemory(input_ptr, input_len, url_ptr, encoding_ptr, options);
        if docptr.is_null() {
          Err(XmlParseError::GotNullPointer)
        } else {
          Ok(Document::new_ptr(docptr))
        }
      },
      ParseFormat::HTML => unsafe {
        let docptr = htmlReadMemory(input_ptr, input_len, url_ptr, encoding_ptr, options);
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
  pub fn is_well_formed_html_with_encoding<Bytes: AsRef<[u8]>>(
    &self,
    input: Bytes,
    encoding: Option<&str>,
  ) -> bool {
    // Process input string.
    let input_bytes = input.as_ref();
    if input_bytes.is_empty() {
      return false;
    }
    let input_ptr = input_bytes.as_ptr() as *const c_char;
    let input_len = match try_usize_to_i32(input_bytes.len()) {
      Ok(v) => v,
      Err(_) => return false,
    };

    // Process encoding.
    let encoding_cstring: Option<CString> = encoding.map(|v| CString::new(v).unwrap());
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
        let docptr = htmlCtxtReadMemory(ctxt, input_ptr, input_len, url_ptr, encoding_ptr, 10_596); // htmlParserOption = 4+32+64+256+2048+8192
        let well_formed_final = if htmlWellFormed(ctxt) {
          // Basic well-formedness passes, let's check if we have an <html> element as root too
          if !docptr.is_null() {
            let node_ptr = xmlDocGetRootElement(docptr);
            if node_ptr.is_null() {
              return false
            }
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
