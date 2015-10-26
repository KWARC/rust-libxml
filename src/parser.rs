//! The parser functionality

use c_signatures::*;
use tree::*;

use std::ffi::CString;
use std::fmt;

enum HtmlParserOption {
    HtmlParseRecover = 1, // Relaxed parsing
    // HTML_PARSE_NODEFDTD = 4, // do not default a doctype if not found
    HtmlParseNoerror = 32, // suppress error reports
    HtmlParseNowarning = 64, // suppress warning reports
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
            XmlParseError::GotNullPointer => write!(f, "Got a Null pointer")
        }
    }
}


impl Document {
    ///Parses the XML file `filename` to generate a new `Document`
    pub fn parse_file(filename : &str) -> Result<Document, XmlParseError> {
        let c_filename = CString::new(filename).unwrap().as_ptr();
        unsafe {
            let docptr = xmlParseFile(c_filename);
            if docptr.is_null() {
                return Err(XmlParseError::GotNullPointer);
            }
            Ok(Document {
                doc_ptr : docptr
            })
        }
    }

    ///Parses the HTML file `filename` to generate a new `Document`
    pub fn parse_html_file(filename : &str) -> Result<Document, XmlParseError> {
        let c_filename = CString::new(filename).unwrap().as_ptr();
        // TODO: Allow user-specified options later on
        let options : u32 = HtmlParserOption::HtmlParseRecover as u32 +
          HtmlParserOption::HtmlParseNoerror as u32 +
          HtmlParserOption::HtmlParseNowarning as u32;

        unsafe {
            let docptr = htmlReadFile(c_filename, CString::new("utf-8").unwrap().as_ptr(), options);
            if docptr.is_null() {
                return Err(XmlParseError::GotNullPointer);
            }
            Ok(Document {
                doc_ptr : docptr
            })
        }
    }
}


///Free global memory of parser
///Do not call this function before all parsing is done!
pub fn xml_cleanup_parser() {
    unsafe {
        xmlCleanupParser();
    }
}
