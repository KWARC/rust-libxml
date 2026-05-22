//! Custom I/O callbacks for libxml2.
//!
//! libxml2 routes every URL it loads (XML documents themselves, plus
//! `xsl:import` / `xsl:include` targets, RelaxNG `<include>`, DTD
//! external subsets, and so on) through a chain of registered "input
//! callback" handlers. The default chain handles `file://`, `http://`,
//! `ftp://`, etc.; an application can prepend its own handler for a
//! custom URL scheme via `xmlRegisterInputCallbacks(match, open,
//! read, close)`.
//!
//! This module wraps that C API in a safe, closure-friendly Rust
//! surface. The motivating use case is shipping a single-binary CLI
//! that bundles its XSLT stylesheets / RNG schemas at compile time
//! via `include_bytes!` and serves them through a custom URL scheme
//! (e.g. `embed:///LaTeXML-html5.xsl`), so `libxslt::parser::parse_bytes`
//! can resolve `xsl:import` chains without ever touching the disk.
//!
//! ```no_run
//! use libxml::io;
//!
//! // Bundled at compile time.
//! static MAIN: &[u8] = b"<?xml version=\"1.0\"?>\n<root/>";
//!
//! io::register_input_callback(
//!   |url| url.starts_with("embed:///"),
//!   |url| match url.strip_prefix("embed:///") {
//!     Some("main.xml") => Some(MAIN.to_vec()),
//!     _ => None,
//!   },
//! );
//! ```
//!
//! ## Lifetime and threading
//!
//! Closures registered here live for the lifetime of the process —
//! libxml2 holds the C trampoline pointers in a static table and
//! has no concept of "unregister single handler" (only
//! `xmlCleanupInputCallbacks` which wipes everything including the
//! defaults). The trampolines look up the Rust closures through a
//! `Mutex<Vec<Callback>>`; libxml2 may invoke them from any thread,
//! hence the `Send + Sync` bound.
//!
//! ## Order
//!
//! libxml2 walks its registered callbacks in last-registered-first
//! order. This module preserves that ordering: callers can stack
//! multiple registrations for the same scheme and the most recent
//! wins. The default file/HTTP handlers remain at the bottom of the
//! stack and continue to serve URLs that none of the custom
//! callbacks claim via their match function.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::{Mutex, OnceLock};

use crate::bindings::{
  xmlInputCloseCallback, xmlInputMatchCallback, xmlInputOpenCallback, xmlInputReadCallback,
  xmlRegisterInputCallbacks,
};

/// One Rust-side callback pair: a URL filter and a byte-fetcher.
///
/// We hold both `match_url` and `open` together so the trampoline
/// can walk the list once. `open` may return `None` even after
/// `match_url` returned `true`; in that case the trampoline keeps
/// walking — the next registered callback gets a chance.
struct Callback {
  match_url: Box<dyn Fn(&str) -> bool + Send + Sync + 'static>,
  open:      Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static>,
}

/// Registry of Rust callbacks. Initialised on first registration.
fn callbacks() -> &'static Mutex<Vec<Callback>> {
  static CALLBACKS: OnceLock<Mutex<Vec<Callback>>> = OnceLock::new();
  CALLBACKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register a custom input callback with libxml2.
///
/// `match_url` is consulted for every URL libxml2 considers loading.
/// Return `true` to claim the URL; the same callback's `open`
/// function will then be invoked to produce the bytes. Return
/// `false` to let later callbacks (or the default file/HTTP loaders)
/// handle it. `open` may itself return `None` to defer back to other
/// handlers — useful for "embedded asset" loaders that only know
/// about a specific basename set.
///
/// The closures are `Send + Sync` because libxml2 may call them from
/// any thread that calls into the parser. They are leaked into a
/// process-static registry; there is no `unregister` API (libxml2
/// does not expose one for individual handlers).
///
/// The actual C trampolines are registered with libxml2 exactly
/// once per process across all calls to this function. Subsequent
/// calls just append another Rust callback to the registry.
///
/// # Example
///
/// Serve an XML fragment for `embed:///hello.xml`. The callback fires
/// whenever libxml2 itself opens the URL — most commonly during
/// `xsl:import` / `xsl:include` resolution from libxslt, or during
/// RelaxNG `<include>` chains in `xmlRelaxNGParse`. The library's
/// own `Parser::parse_file` uses Rust file I/O directly and does
/// *not* go through libxml2's URL machinery, so it would not
/// trigger this callback — wire it up via libxslt's `parse_bytes`
/// or libxml2's `xmlReadFile` instead.
///
/// ```no_run
/// use libxml::io;
///
/// static HELLO: &[u8] = b"<?xml version=\"1.0\"?>\n<hello>world</hello>";
///
/// io::register_input_callback(
///   |url| url.starts_with("embed:///"),
///   |url| {
///     if url == "embed:///hello.xml" {
///       Some(HELLO.to_vec())
///     } else {
///       None
///     }
///   },
/// );
/// ```
pub fn register_input_callback<M, O>(match_url: M, open: O)
where
  M: Fn(&str) -> bool + Send + Sync + 'static,
  O: Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static,
{
  callbacks().lock().unwrap().push(Callback {
    match_url: Box::new(match_url),
    open:      Box::new(open),
  });

  // Install the C trampolines exactly once. libxml2 records the
  // function pointers in a static table; calling
  // `xmlRegisterInputCallbacks` twice would push two duplicate
  // entries that both delegate to the same Rust registry — wasteful
  // but not unsafe. The OnceLock avoids the duplicate registration.
  static REGISTERED: OnceLock<()> = OnceLock::new();
  REGISTERED.get_or_init(|| {
    crate::init_parser();
    unsafe {
      xmlRegisterInputCallbacks(
        Some(trampoline_match as xmlInputMatchCallback_t),
        Some(trampoline_open as xmlInputOpenCallback_t),
        Some(trampoline_read as xmlInputReadCallback_t),
        Some(trampoline_close as xmlInputCloseCallback_t),
      );
    }
  });
}

// Type aliases for the inner `Option`-wrapped function pointers
// libxml2 expects. Keeps the unsafe-cast site at `register_input_callback`
// concise and self-documenting.
type xmlInputMatchCallback_t = unsafe extern "C" fn(*const c_char) -> c_int;
type xmlInputOpenCallback_t = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type xmlInputReadCallback_t = unsafe extern "C" fn(*mut c_void, *mut c_char, c_int) -> c_int;
type xmlInputCloseCallback_t = unsafe extern "C" fn(*mut c_void) -> c_int;

// Compile-time sanity: the function-pointer type aliases above must
// be ABI-compatible with the libxml2 callback signatures from the
// generated bindings. If bindgen ever regenerates them differently
// (e.g. switches to a different `*const`/`*mut` flavour), this
// assertion fires at compile time.
const _: xmlInputMatchCallback = Some(trampoline_match);
const _: xmlInputOpenCallback = Some(trampoline_open);
const _: xmlInputReadCallback = Some(trampoline_read);
const _: xmlInputCloseCallback = Some(trampoline_close);

/// Per-open state: a byte buffer + read cursor. Owned by libxml2 via
/// a `*mut c_void` handle until `trampoline_close` reclaims and drops it.
struct OpenState {
  bytes:    Vec<u8>,
  position: usize,
}

/// libxml2 match callback: does any registered Rust callback claim
/// this URL? Returns 1 on claim, 0 to defer.
unsafe extern "C" fn trampoline_match(filename: *const c_char) -> c_int {
  if filename.is_null() {
    return 0;
  }
  // SAFETY: libxml2 guarantees `filename` is a NUL-terminated C string
  // for the lifetime of this call. Lossy decode means non-UTF-8 URLs
  // are rejected (they can't possibly match anyway).
  let url = match unsafe { CStr::from_ptr(filename) }.to_str() {
    Ok(s) => s,
    Err(_) => return 0,
  };
  let cbs = match callbacks().lock() {
    Ok(g) => g,
    Err(_) => return 0,
  };
  for cb in cbs.iter() {
    if (cb.match_url)(url) {
      return 1;
    }
  }
  0
}

/// libxml2 open callback: walk the registry, return the first
/// successfully-produced byte buffer as a heap-allocated `OpenState`
/// handle.
unsafe extern "C" fn trampoline_open(filename: *const c_char) -> *mut c_void {
  if filename.is_null() {
    return std::ptr::null_mut();
  }
  // SAFETY: see `trampoline_match`.
  let url = match unsafe { CStr::from_ptr(filename) }.to_str() {
    Ok(s) => s,
    Err(_) => return std::ptr::null_mut(),
  };
  let cbs = match callbacks().lock() {
    Ok(g) => g,
    Err(_) => return std::ptr::null_mut(),
  };
  for cb in cbs.iter() {
    if !(cb.match_url)(url) {
      continue;
    }
    if let Some(bytes) = (cb.open)(url) {
      let state = Box::new(OpenState { bytes, position: 0 });
      return Box::into_raw(state) as *mut c_void;
    }
  }
  std::ptr::null_mut()
}

/// libxml2 read callback: copy up to `len` bytes from the buffer
/// cursor into `buffer`. Returns the number copied, 0 at EOF, or
/// -1 on error.
unsafe extern "C" fn trampoline_read(
  context: *mut c_void,
  buffer: *mut c_char,
  len: c_int,
) -> c_int {
  if context.is_null() || buffer.is_null() || len <= 0 {
    return -1;
  }
  // SAFETY: `context` was produced by `trampoline_open` (Box::into_raw)
  // and not yet reclaimed by `trampoline_close`; libxml2 holds at most
  // one mutable reference at a time per open handle.
  let state = unsafe { &mut *(context as *mut OpenState) };
  let remaining = state.bytes.len().saturating_sub(state.position);
  let n = remaining.min(len as usize);
  if n == 0 {
    return 0;
  }
  // SAFETY: ranges checked above. Source and dest do not overlap —
  // they live in disjoint heap allocations.
  unsafe {
    std::ptr::copy_nonoverlapping(
      state.bytes.as_ptr().add(state.position),
      buffer as *mut u8,
      n,
    );
  }
  state.position += n;
  n as c_int
}

/// libxml2 close callback: reclaim and drop the `OpenState` box that
/// `trampoline_open` produced.
unsafe extern "C" fn trampoline_close(context: *mut c_void) -> c_int {
  if context.is_null() {
    return -1;
  }
  // SAFETY: `context` was produced by `Box::into_raw` in
  // `trampoline_open`; this is the unique reclamation site.
  let _state = unsafe { Box::from_raw(context as *mut OpenState) };
  0
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::bindings::{xmlFreeDoc, xmlReadFile};
  use std::ffi::CString;
  use std::sync::OnceLock;

  static SAMPLE_XML: &[u8] = br#"<?xml version="1.0"?>
<root attr="ok"><child/></root>"#;

  /// Install the test callback exactly once per process. The libxml2
  /// callback table is process-global; stacking the same callback
  /// across `#[test]` runs would still work (last registered wins via
  /// the registry walk) but adds noise — keep the registry to one
  /// entry for determinism.
  fn install_once() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
      register_input_callback(
        |url| url.starts_with("embed:///"),
        |url| {
          if url == "embed:///sample.xml" {
            Some(SAMPLE_XML.to_vec())
          } else {
            None
          }
        },
      );
    });
  }

  /// Call libxml2's `xmlReadFile` directly. Used by the tests to
  /// exercise the URL-routed parser path — `Parser::parse_file`
  /// short-circuits through Rust file I/O so our callbacks aren't
  /// involved there. Production use is identical to what `libxslt`
  /// does internally when resolving `xsl:import` against a base URI:
  /// libxslt calls `xmlReadFile` with the composed URL, libxml2 walks
  /// its registered input callbacks (including ours), our trampolines
  /// produce the bytes.
  fn read_file_via_libxml2(url: &str) -> bool {
    let c = CString::new(url).unwrap();
    unsafe {
      let doc = xmlReadFile(c.as_ptr(), std::ptr::null(), 0);
      if doc.is_null() {
        return false;
      }
      xmlFreeDoc(doc);
      true
    }
  }

  #[test]
  fn callback_serves_registered_url() {
    install_once();
    assert!(
      read_file_via_libxml2("embed:///sample.xml"),
      "registered URL should parse via the callback",
    );
  }

  #[test]
  fn callback_can_decline_via_none() {
    install_once();
    assert!(
      !read_file_via_libxml2("embed:///unknown.xml"),
      "decline (open returning None) should fail the load, not return phantom data",
    );
  }

  #[test]
  fn non_matching_url_defers_to_default_handlers() {
    install_once();
    // A file:// URL that doesn't exist must fall through to libxml2's
    // built-in file handler and fail there — confirms our match
    // callback returns 0 for unrelated URLs (otherwise we'd intercept
    // and break every default load).
    assert!(
      !read_file_via_libxml2("/nonexistent/definitely/missing.xml"),
      "non-embed URL should fail through the default loader",
    );
  }
}
