//! Leak checks that need no valgrind. libxml2's allocations are routed through
//! counting wrappers (`xmlMemSetup`) and Rust's through a counting global
//! allocator; each check repeats an operation and asserts that the number of
//! live allocations does not grow.
//!
//! Everything runs in one test: other tests in this binary would allocate
//! concurrently and disturb the counts.
use std::alloc::{GlobalAlloc, Layout, System};
use std::os::raw::{c_char, c_void};
use std::sync::atomic::{AtomicIsize, Ordering};

use libxml::parser::Parser;
use libxml::tree::c14n::{CanonicalizationMode, CanonicalizationOptions};
use libxml::xpath::is_well_formed_xpath;

static RUST_LIVE: AtomicIsize = AtomicIsize::new(0);
static LIBXML_LIVE: AtomicIsize = AtomicIsize::new(0);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let ptr = unsafe { System.alloc(layout) };
    if !ptr.is_null() {
      RUST_LIVE.fetch_add(1, Ordering::Relaxed);
    }
    ptr
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    RUST_LIVE.fetch_sub(1, Ordering::Relaxed);
    unsafe { System.dealloc(ptr, layout) }
  }
}

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

unsafe extern "C" fn count_malloc(size: usize) -> *mut c_void {
  let ptr = unsafe { libc::malloc(size) };
  if !ptr.is_null() {
    LIBXML_LIVE.fetch_add(1, Ordering::Relaxed);
  }
  ptr
}

unsafe extern "C" fn count_realloc(old: *mut c_void, size: usize) -> *mut c_void {
  let ptr = unsafe { libc::realloc(old, size) };
  if old.is_null() && !ptr.is_null() {
    LIBXML_LIVE.fetch_add(1, Ordering::Relaxed);
  }
  ptr
}

unsafe extern "C" fn count_free(ptr: *mut c_void) {
  if !ptr.is_null() {
    LIBXML_LIVE.fetch_sub(1, Ordering::Relaxed);
  }
  unsafe { libc::free(ptr) }
}

// Built on `count_malloc` rather than `strdup`, which MSVC spells `_strdup`.
unsafe extern "C" fn count_strdup(s: *const c_char) -> *mut c_char {
  unsafe {
    let len = libc::strlen(s) + 1;
    let ptr = count_malloc(len) as *mut c_char;
    if !ptr.is_null() {
      std::ptr::copy_nonoverlapping(s, ptr, len);
    }
    ptr
  }
}

/// Live allocations added by `rounds` further runs of `op`, after two warm-up
/// runs (lazy initialisation inside libxml2 is not a leak).
fn growth(counter: &AtomicIsize, rounds: usize, mut op: impl FnMut()) -> isize {
  op();
  op();
  let before = counter.load(Ordering::SeqCst);
  for _ in 0..rounds {
    op();
  }
  counter.load(Ordering::SeqCst) - before
}

#[test]
fn repeated_operations_do_not_leak() {
  // Must precede any libxml2 allocation, so before anything initialises it.
  let rc = unsafe {
    libxml::bindings::xmlMemSetup(
      Some(count_free),
      Some(count_malloc),
      Some(count_realloc),
      Some(count_strdup),
    )
  };
  assert_eq!(rc, 0, "xmlMemSetup failed");
  libxml::init_parser();

  // The compiled expression was released with a plain `free`, leaking its steps.
  let xpath = growth(&LIBXML_LIVE, 100, || {
    assert!(is_well_formed_xpath("//a[@b='c']/d | //e"));
  });
  assert_eq!(
    xpath, 0,
    "is_well_formed_xpath leaked {xpath} libxml2 allocations in 100 calls"
  );

  // The inclusive namespace prefixes were handed to libxml2 as leaked `CString`s.
  let doc = Parser::default()
    .parse_string(r#"<r xmlns:a="urn:a" xmlns:b="urn:b"><a:x b:y="1"/></r>"#)
    .unwrap();
  // Guard against a vacuous pass: the live document's nodes must be counted.
  assert!(
    LIBXML_LIVE.load(Ordering::SeqCst) > 0,
    "libxml2 allocations are not being counted"
  );
  let canonicalize = || {
    let options = CanonicalizationOptions {
      mode: CanonicalizationMode::ExclusiveCanonical1_0,
      with_comments: false,
      inclusive_ns_prefixes: vec!["a".to_string(), "b".to_string()],
    };
    assert!(!doc.canonicalize(options, None).unwrap().is_empty());
  };
  let rust = growth(&RUST_LIVE, 50, canonicalize);
  assert_eq!(
    rust, 0,
    "canonicalize leaked {rust} Rust allocations in 50 calls"
  );
  let libxml = growth(&LIBXML_LIVE, 50, canonicalize);
  assert_eq!(
    libxml, 0,
    "canonicalize leaked {libxml} libxml2 allocations in 50 calls"
  );
}
