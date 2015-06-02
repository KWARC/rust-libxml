//! # A wrapper for libxml2
//! This library provides an interface to a subset of the libxml API.
//! The idea is to extend it whenever more functionality is needed.
//! Providing a more or less complete wrapper would be too much work.

extern crate libc;

use std::ffi::CString;
use std::fmt;

use libc::{c_char, c_void, c_int};

#[link(name = "xml2")]
extern "C" {
    fn xmlParseFile(filename: *const c_char) -> *mut c_void;
    fn xmlSaveFile(filename: *const c_char, cur: *mut c_void) -> c_int;
}

///An xml document
pub struct XmlDoc {
    ///It's libxml's xmlDocPtr
    xml_doc_ptr : *mut c_void,
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

impl XmlDoc {
    ///parses the file `filename` to generate a new `XmlDoc`
    pub fn parse_file(filename : &str) -> std::result::Result<XmlDoc, XmlParseError> {
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

    ///Write document to `filename`
    pub fn save_file(&self, filename : &str) -> Result<c_int, ()> {
        let c_filename = CString::new(filename).unwrap().as_ptr();
        unsafe {
            let retval = xmlSaveFile(c_filename, self.xml_doc_ptr);
            if retval < 0 {
                return Err(());
            }
            Ok(retval)
        }
    }
}

