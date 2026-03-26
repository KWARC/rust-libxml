use std::{env, fs, path::PathBuf};

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

  let host = env::var("HOST").unwrap();

  let mut libs = std::process::Command::new("sh")
    .arg(path.join("bin/xml2-config"))
    .arg("--libs")
    .output()
    .map(|output| String::from_utf8_lossy(&output.stdout).to_string());
  let mut cflags = std::process::Command::new("sh")
    .arg(path.join("bin/xml2-config"))
    .arg("--cflags")
    .output()
    .map(|output| String::from_utf8_lossy(&output.stdout).to_string());

  if host.contains("windows") {
    let reg = regex::Regex::new("-(.)/(.)/").expect("reg");
    libs = libs.map(|v| reg.replace_all(&v, "-$1$2:/").to_string());
    cflags = cflags.map(|v| reg.replace_all(&v, "-$1$2:/").to_string());
  }
  // NOTE: Manually specify
  let mut libs = libs.unwrap_or_else(|_| format!("-L{} -lxml2", path.join("lib").display()));
  if host.contains("msvc") {
    let mut iters = fs::read_dir(path.join("lib"))
      .expect("read_dir")
      .filter_map(|p| {
        p.ok().and_then(|p| {
          let metadata = p.metadata().ok()?;
          let file_name = p.file_name();
          let name = file_name.to_string_lossy();
          if metadata.is_file() && name.starts_with("libxml2") && name.ends_with(".lib") {
            return Some(
              name
                .trim_end_matches(".lib")
                .to_string(),
            );
          }
          None
        })
      });
    let name = iters.next().expect("xml name");
    println!("cargo:rustc-link-lib=bcrypt");
    libs = libs.replace("-lxml2", &format!("-l{}", name));
  }
  println!("cargo::rustc-flags={}", libs);

  // Note: Manually specify
  let cflags = cflags.unwrap_or_else(|_| format!("-I{}", path.join("include/libxml2").display()));
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
