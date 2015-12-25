//! We'll have to compile our helper_functions.c and create the
//! static library libhelper_functions.a
//! We'll assume that `gcc` and `ar` are installed and that the
//! header files are in `/usr/include/libxml2`.
//! In the future, we should move to a more flexible solution.

extern crate gcc;

fn main() {
  gcc::Config::new()
    .file("src/helper_functions.c")
    .include("/usr/include/libxml2")
    .compile("libhelper_functions.a");
}

