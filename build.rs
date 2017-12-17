//! We'll have to compile our helper_functions.c and create the
//! static library libhelper_functions.a
//! We'll assume that `gcc` and `ar` are installed and that the
//! header files are in `/usr/include/libxml2`.
//! In the future, we should move to a more flexible solution.

/*
extern crate gcc;

fn main() {
  gcc::Build::new()
    .file("src/helper_functions.c")
    .include("/usr/include/libxml2")
    .compile("libhelper_functions.a");
}
*/

extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
  // Tell cargo to tell rustc to link the system bzip2
  // shared library.
  println!("cargo:rustc-link-lib=xml2");

  // The bindgen::Builder is the main entry point
  // to bindgen, and lets you build up options for
  // the resulting bindings.
  let bindings = bindgen::Builder::default()
      // The input header we would like to generate
      // bindings for.
      .header("src/libxml2/wrapper.h")
      // Homebrew location of libxml2 headers.
      .clang_arg("-I/usr/include/libxml2")
      // Finish the builder and generate the bindings.
      .generate()
      // Unwrap the Result and panic on failure.
      .expect("Unable to generate bindings");

  // Write the bindings to the $OUT_DIR/bindings.rs file.
  //let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
  bindings
      //.write_to_file(out_path.join("bindings.rs"))
      .write_to_file("src/libxml2/bindings.rs")
      .expect("Couldn't write bindings!");
}
