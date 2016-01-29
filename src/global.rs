/// We are going to handle the global init+cleanup of libxml in tree, for now.
use c_signatures::*;

use std::sync::{StaticMutex, MUTEX_INIT};

static LIBXML_LOCK: StaticMutex = MUTEX_INIT;
static mut LIBXML_OBJECTS: i64 = 0;
pub fn _libxml_global_init() {
  let _g = LIBXML_LOCK.lock().unwrap();
  unsafe {
    if LIBXML_OBJECTS == 0 {
      xmlInitParser();
      xmlInitGlobals();
    }
    LIBXML_OBJECTS += 1;
  }
}
pub fn _libxml_global_drop() {
  let _g = LIBXML_LOCK.lock().unwrap();
  unsafe {
    if LIBXML_OBJECTS == 1 { // Far from perfect, more "desperate" than anything...
      // The big issue here is taht calling these deallocations causes segfaults
      // if any thread is still using libxml2 constructs. And Rust doesn't give us a good hook for "end of static scope",
      // hence we're sunk... For now uncommenting here and leaving a "libxml_force_global_drop()" for manual use...
      //  ... very unsatisfying

      //xmlCleanupGlobals();
      //xmlCleanupParser();
    }
    LIBXML_OBJECTS -= 1;
  }
}

pub fn force_global_drop() {
  unsafe {
    xmlCleanupGlobals();
    xmlCleanupParser();
  }
}