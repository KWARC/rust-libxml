//! libxml2's heap behind `xmlMemSetup`, with every block it frees kept in quarantine: zeroed and never returned to the
//! system, so its address is not reused while the test binary runs. A pointer into such a block is then recognizably
//! dangling, without valgrind: [`is_freed`] answers whether libxml2 has freed the block at an address, and
//! [`double_freed`] whether it freed it twice (a second free is recorded, not performed).
#![allow(dead_code)]
use std::collections::{HashMap, HashSet};
use std::os::raw::{c_char, c_void};
use std::sync::{LazyLock, Mutex, MutexGuard, Once};

#[derive(Default)]
struct Heap {
  /// Size of each live block, by address.
  live: HashMap<usize, usize>,
  freed: HashSet<usize>,
  double_freed: HashSet<usize>,
}

static HEAP: LazyLock<Mutex<Heap>> = LazyLock::new(Default::default);

fn heap() -> MutexGuard<'static, Heap> {
  HEAP.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn quarantine(heap: &mut Heap, ptr: *mut c_void) {
  let address = ptr as usize;
  if !heap.freed.insert(address) {
    heap.double_freed.insert(address);
    return;
  }
  if let Some(size) = heap.live.remove(&address) {
    // SAFETY: `ptr` is a block of `size` bytes this heap handed out, now freed by libxml2.
    unsafe { std::ptr::write_bytes(ptr as *mut u8, 0, size) };
  }
}

unsafe extern "C" fn quarantine_malloc(size: usize) -> *mut c_void {
  let ptr = unsafe { libc::malloc(size.max(1)) };
  if !ptr.is_null() {
    let mut heap = heap();
    heap.live.insert(ptr as usize, size);
    heap.freed.remove(&(ptr as usize));
  }
  ptr
}

unsafe extern "C" fn quarantine_free(ptr: *mut c_void) {
  if !ptr.is_null() {
    quarantine(&mut heap(), ptr);
  }
}

unsafe extern "C" fn quarantine_realloc(old: *mut c_void, size: usize) -> *mut c_void {
  if old.is_null() {
    return unsafe { quarantine_malloc(size) };
  }
  let old_size = heap().live.get(&(old as usize)).copied();
  let Some(old_size) = old_size else {
    // Not a block of this heap: allocated before `setup`, which runs before libxml2 allocates anything.
    return unsafe { libc::realloc(old, size) };
  };
  let new = unsafe { quarantine_malloc(size) };
  if !new.is_null() {
    // SAFETY: both blocks are live, and at least `old_size.min(size)` bytes long.
    unsafe { std::ptr::copy_nonoverlapping(old as *const u8, new as *mut u8, old_size.min(size)) };
    quarantine(&mut heap(), old);
  }
  new
}

// Built on `quarantine_malloc` rather than `strdup`, which MSVC spells `_strdup`.
unsafe extern "C" fn quarantine_strdup(s: *const c_char) -> *mut c_char {
  unsafe {
    let len = libc::strlen(s) + 1;
    let ptr = quarantine_malloc(len) as *mut c_char;
    if !ptr.is_null() {
      std::ptr::copy_nonoverlapping(s, ptr, len);
    }
    ptr
  }
}

/// Routes libxml2's heap through the quarantine. Every test calls it first: it must precede libxml2's first
/// allocation.
pub fn setup() {
  static ONCE: Once = Once::new();
  ONCE.call_once(|| {
    let rc = unsafe {
      libxml::bindings::xmlMemSetup(
        Some(quarantine_free),
        Some(quarantine_malloc),
        Some(quarantine_realloc),
        Some(quarantine_strdup),
      )
    };
    assert_eq!(rc, 0, "xmlMemSetup failed");
    libxml::init_parser();
  });
}

/// Whether libxml2 has freed the block at `ptr`. A null pointer never is.
pub fn is_freed<T>(ptr: *const T) -> bool {
  heap().freed.contains(&(ptr as usize))
}

/// Whether libxml2 has freed the block at `ptr` more than once.
pub fn double_freed<T>(ptr: *const T) -> bool {
  heap().double_freed.contains(&(ptr as usize))
}
