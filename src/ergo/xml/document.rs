use std::ffi::{ CString };
use libxml2::{xmlResetLastError, xmlSetStructuredErrorFunc, xmlReadMemory, xmlReadFile, xmlFreeDoc, _xmlDoc, _xmlNode, xmlDocGetRootElement};
use std::mem;
use std::ptr;
use std::os::raw::c_void;

use ergo::xml::{ Node, XmlInput, XmlError, ParseOptions, error_vec_pusher };

pub struct Document {
    pub doc_ptr: *mut _xmlDoc,
    pub errors: Vec<XmlError>,
}

impl Document {
    /// Get the root element of the document
    pub fn get_root_element(&self) -> Node {
        unsafe {
            let node_ptr = xmlDocGetRootElement(self.doc_ptr);
            if node_ptr.is_null() {
                Node {
                    node_ptr: self.doc_ptr as *mut _xmlNode
                }
            } else {
                Node {
                    node_ptr : node_ptr,
                }
            }
        }
    }
    pub fn new_ptr(doc_ptr: *mut _xmlDoc) -> Self {
        Document {
            doc_ptr,
            errors: vec![],
        }
    }
    pub fn parse<R: XmlInput + ?Sized>(r:&R) -> Result<Document, Vec<XmlError>> {
        Document::parse_with_options(r, "", "utf-8", ParseOptions::DEFAULT_XML)
    }

    pub fn parse_with_options<R: XmlInput + ?Sized>(r:&R, url: &str, encoding: &str, options: ParseOptions) -> Result<Document, Vec<XmlError>> {
        match r.is_path() {
            true => Document::parse_file(&r.data(), encoding, options),
            false => Document::parse_string(&r.data(), url, encoding, options)
        }
    }

    fn parse_string(xml_str: &str, url: &str, encoding: &str, options: ParseOptions) -> Result<Document, Vec<XmlError>> {
        let c_string_len = xml_str.len() as i32;
        let c_string = CString::new(xml_str).unwrap();
        let c_utf8 = CString::new(encoding).unwrap();
        let c_url = CString::new(url).unwrap();
        Document::parse_handler(|| unsafe { xmlReadMemory(c_string.as_ptr(), c_string_len, c_url.as_ptr(), c_utf8.as_ptr(), options.bits as i32) })
    }

    fn parse_file(filename: &str, encoding: &str, options: ParseOptions) -> Result<Document, Vec<XmlError>> {
        let c_filename = CString::new(filename).unwrap();
        let c_utf8 = CString::new(encoding).unwrap();
        Document::parse_handler(|| unsafe { xmlReadFile(c_filename.as_ptr(), c_utf8.as_ptr(), options.bits as i32) })
    }

    fn parse_handler<F>(parse_closure: F) -> Result<Document, Vec<XmlError>> where F: Fn() -> *mut _xmlDoc {
        unsafe {
            let errors: Box<Vec<XmlError>> = Box::new(vec![]);
            xmlResetLastError();
            let errors_ptr: *mut c_void = mem::transmute(errors);
            xmlSetStructuredErrorFunc(errors_ptr, Some(error_vec_pusher));
            let doc_ptr = parse_closure();
            xmlSetStructuredErrorFunc(ptr::null_mut(), None);
            Document::handle_result_ptrs(doc_ptr, errors_ptr)
        }
    }

    fn handle_result_ptrs(doc_ptr: *mut _xmlDoc, errors_ptr: *mut c_void) -> Result<Document, Vec<XmlError>> {
        let errors: Box<Vec<XmlError>> = unsafe { mem::transmute(errors_ptr) };
        match doc_ptr.is_null() {
            true => {
                unsafe { xmlFreeDoc(doc_ptr) };

                // Nokogiri raises the last error, not sure what we want or what would be idiomatic.
                //Err(xml_get_last_error())

                Err(*errors)
            }
            false => {
                // TODO: Implement XInclude

                /* attache *errors to document */
                let mut doc = Document::new_ptr(doc_ptr);
                doc.errors = *errors;
                Ok(doc)
            }
        }
    }

    /*
    fn xml_get_last_error() -> Vec<XmlError> {
        let err_ptr = unsafe { xmlGetLastError() };
        match err_ptr.is_null() {
            true => panic!("Boom! err_ptr is null"),
            false => {
                let err = convert(err_ptr);

                // TODO: find out why this fails in test with (signal: 11, SIGSEGV: invalid memory reference)
                //let err_v = vec![err];


                let mut err_v = vec![];
                err_v.push(err);
                err_v
            }
        }
    }

    fn convert(libxml_error: *mut _xmlError) -> XmlError {
        unsafe {
            let msg = CStr::from_ptr((*libxml_error).message).to_str().expect("Failed to get error msg");
            XmlError {message: String::from(msg)}
        }
    }
    */

}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_string_test(){
        assert_eq ! (true, Document::parse_string("a><root></root>", "", "utf-8", ParseOptions::DEFAULT_XML).is_ok());
    }
}
