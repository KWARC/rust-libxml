use pkg_config::find_library;

fn main() {
  if find_library("libxml-2.0").is_err() {
    panic!("Could not find libxml2 using pkg-config")
  }
}
