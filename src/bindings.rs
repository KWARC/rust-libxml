// Issues coming from bindgen
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(improper_ctypes)]
#![allow(missing_docs)]

/*
 * helper var until we figure out well-formedness checks
 */

pub static mut HACKY_WELL_FORMED: bool = false;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
