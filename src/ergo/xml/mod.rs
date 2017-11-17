use std::ffi::{ CStr };
use libxml2::{ _xmlError };
use std::os::raw::c_void;
use std::mem;

use std::io::{Read, BufReader};
use std::fs::File;
use std::path::Path;

pub mod document;

bitflags! {
    pub struct ParseOptions: i32 {
        // Strict parsing
        const STRICT      = 0;
        // Recover from errors
        const RECOVER     = 1 << 0;
        // Substitute entities
        const NOENT       = 1 << 1;
        // Load external subsets
        const DTDLOAD     = 1 << 2;
        // Default DTD attributes
        const DTDATTR     = 1 << 3;
        // validate with the DTD
        const DTDVALID    = 1 << 4;
        // suppress error reports
        const NOERROR     = 1 << 5;
        // suppress warning reports
        const NOWARNING   = 1 << 6;
        // pedantic error reporting
        const PEDANTIC    = 1 << 7;
        // remove blank nodes
        const NOBLANKS    = 1 << 8;
        // use the SAX1 interface internally
        const SAX1        = 1 << 9;
        // Implement XInclude substitution
        const XINCLUDE    = 1 << 10;
        // Forbid network access. Recommended for dealing with untrusted documents.
        const NONET       = 1 << 11;
        // Do not reuse the context dictionary
        const NODICT      = 1 << 12;
        // remove redundant namespaces declarations
        const NSCLEAN     = 1 << 13;
        // merge CDATA as text nodes
        const NOCDATA     = 1 << 14;
        // do not generate XINCLUDE START/END nodes
        const NOXINCNODE  = 1 << 15;
        // compact small text nodes; no modification of the tree allowed afterwards (will possibly crash if you try to modify the tree)
        const COMPACT     = 1 << 16;
        // parse using XML-1.0 before update 5;
        const OLD10       = 1 << 17;
        // do not fixup XINCLUDE xml:base uris
        const NOBASEFIX   = 1 << 18;
        // relax any hardcoded limit from the parser
        const HUGE        = 1 << 19;
        // the default options used for parsing XML documents
        const DEFAULT_XML  = Self::RECOVER.bits
            | Self::NONET.bits;
        // the default options used for parsing HTML documents
        const DEFAULT_HTML = Self::RECOVER.bits
            | Self::NOERROR.bits
            | Self::NOWARNING.bits
            | Self::NONET.bits;
    }
}

#[derive(Debug)]
pub struct XmlError {
    pub message: String
}
pub trait XmlInput {
    const IS_PATH: bool = false;
    fn is_path(&self) -> bool { Self::IS_PATH }
    fn data(&self) -> String;
}

impl XmlInput for str {
    fn data(&self) -> String {
        String::from(self)
    }
}

impl XmlInput for String {
    fn data(&self) -> String {
        self.to_owned()
    }
}

impl XmlInput for Path {
    const IS_PATH: bool = true;
    fn data(&self) -> String {
        String::from(self.to_str().expect("Could not get path"))
    }
}

// TODO: Here we could possibly use the file descriptor if detect *nix.
impl XmlInput for File {
    const IS_PATH: bool = false;
    fn data(&self) -> String {
        let mut tmp = String::new();
        {
            let mut a = BufReader::new(self);
            a.read_to_string(&mut tmp).expect("Could not read_to_string");
        }
        tmp
    }
}

extern "C" fn error_vec_pusher(errors_ptr: *mut c_void, libxml_error: *mut _xmlError) {
    unsafe {
        let msg = CStr::from_ptr((*libxml_error).message).to_str().expect("Failed to get error msg");
        let mut errors: Box<Vec<XmlError>> = mem::transmute(errors_ptr);
        errors.push(XmlError {message: String::from(msg)});
    };
}

