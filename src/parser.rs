//! The parser functionality

use c_signatures::*;
use tree::*;

use std::ffi::CString;
use std::fmt;

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


impl XmlDoc {
    ///Parses the file `filename` to generate a new `XmlDoc`
    pub fn parse_file(filename : &str) -> Result<XmlDoc, XmlParseError> {
        let c_filename = CString::new(filename).unwrap().as_ptr();
        unsafe {
            let docptr = xmlParseFile(c_filename);
            if docptr.is_null() {
                return Err(XmlParseError::GotNullPointer);
            }
            Ok(XmlDoc {
                xml_doc_ptr : docptr
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
