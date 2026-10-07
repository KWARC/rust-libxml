//! `Parser::is_well_formed_html` must not leak state across calls or threads.
//!
//! It tolerates unknown HTML tags (e.g. `<math>`) by watching libxml2's errors
//! through the thread's structured error handler. That handler must be restored
//! afterwards, and what it records must belong to the calling parse only.
use libxml::bindings::__xmlStructuredError;
use libxml::parser::Parser;

const WELL_FORMED_UNKNOWN_TAG: &str =
  "<!DOCTYPE html>\n<html><head><title>T</title></head><body><math><mn>2</mn></math></body></html>";
const ILL_FORMED: &str = "<broken <markup>> </boom>";

#[test]
/// The thread's structured error handler is left as it was found.
fn handler_is_restored() {
  let parser = Parser::default_html();
  let before = unsafe { *__xmlStructuredError() }.map(|f| f as usize);
  assert!(parser.is_well_formed_html(WELL_FORMED_UNKNOWN_TAG));
  assert!(!parser.is_well_formed_html(ILL_FORMED));
  let after = unsafe { *__xmlStructuredError() }.map(|f| f as usize);
  assert_eq!(
    before, after,
    "is_well_formed_html left its error handler installed"
  );
}

#[test]
/// One thread's tolerated unknown tag must not make another thread's malformed
/// input pass.
fn results_do_not_leak_across_threads() {
  let threads: Vec<_> = (0..8)
    .map(|i| {
      std::thread::spawn(move || {
        let parser = Parser::default_html();
        for _ in 0..500 {
          if i % 2 == 0 {
            assert!(parser.is_well_formed_html(WELL_FORMED_UNKNOWN_TAG));
          } else {
            assert!(
              !parser.is_well_formed_html(ILL_FORMED),
              "malformed input passed: another thread's unknown tag leaked in"
            );
          }
        }
      })
    })
    .collect();
  for thread in threads {
    thread.join().unwrap();
  }
}
