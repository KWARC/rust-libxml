/// We are going to handle the global init+cleanup of libxml in tree, for now.
use c_signatures::*;

use std::sync::{StaticMutex, MUTEX_INIT};

static LIBXML_LOCK: StaticMutex = MUTEX_INIT;
static mut LIBXML_OBJECTS: i64 = 0;
pub fn _libxml_global_init() {
  { let _g = LIBXML_LOCK.lock().unwrap();
  unsafe {
    if LIBXML_OBJECTS == 0 {
      xmlInitGlobals();
      xmlInitParser();
    }
    LIBXML_OBJECTS += 1;
  }}
}
pub fn _libxml_global_drop() {
  { let _g = LIBXML_LOCK.lock().unwrap();
  unsafe {
    LIBXML_OBJECTS -= 1;
    if LIBXML_OBJECTS == 0 { // Far from perfect, more "desperate" than anything...
      xmlCleanupParser();
      xmlCleanupGlobals();
    }
  }}
}