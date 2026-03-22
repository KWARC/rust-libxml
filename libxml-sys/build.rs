use std::{env, path::PathBuf};

fn main() {
  let out_dir = env::var("OUT_DIR").unwrap();

  let iconv = if cfg!(feature = "iconv") { "ON" } else { "OFF" };
  let zlib = if cfg!(feature = "zlib") { "ON" } else { "OFF" };
  let path = cmake::Config::new("libxml2")
    .define("BUILD_SHARED_LIBS", "OFF")
    .define("LIBXML2_WITH_ICONV", iconv)
    .define("LIBXML2_WITH_ZLIB", zlib)
    .define("LIBXML2_WITH_C14N", "ON")
    .build();

  println!("cargo::rerun-if-changed=libxml2");

  let libs = std::process::Command::new(format!("{}/bin/xml2-config", path.display(),))
    .arg("--libs")
    .output()
    .expect("");
  let libs = String::from_utf8_lossy(&libs.stdout);
  println!("cargo::rustc-flags={}", libs);

  let cflags = std::process::Command::new(format!("{}/bin/xml2-config", path.display(),))
    .arg("--cflags")
    .output()
    .expect("");
  let cflags = String::from_utf8_lossy(&cflags.stdout);

  let bindings_path = PathBuf::from(out_dir).join("bindings.rs");
  bindgen::builder()
    .opaque_type("max_align_t")
    .header("src/wrapper.h")
    .clang_args(&["-DLIBXML_C14N_ENABLED", "-DLIBXML_OUTPUT_ENABLED"])
    .clang_args(cflags.split_whitespace())
    .generate()
    .expect("failed to generate bindings with bindgen")
    .write_to_file(bindings_path)
    .expect("Failed to write bindings.rs");
}
