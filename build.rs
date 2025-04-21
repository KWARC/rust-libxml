use std::{env, fs, path::{Path, PathBuf}};

struct ProbedLib {
  version: String,
  include_paths: Vec<PathBuf>,
}

/// Finds libxml2 and optionally return a list of header
/// files from which the bindings can be generated.
fn find_libxml2() -> Option<ProbedLib> {
  #![allow(unreachable_code)] // for platform-dependent dead code

  if let Ok(ref s) = std::env::var("LIBXML2") {
    // println!("{:?}", std::env::vars());
    // panic!("set libxml2.");
    let p = std::path::Path::new(s);
    let fname = std::path::Path::new(
      p.file_name()
        .unwrap_or_else(|| panic!("no file name in LIBXML2 env ({s})")),
    );
    assert!(
      p.is_file(),
      "{}",
      &format!("not a file in LIBXML2 env ({s})")
    );
    println!(
      "cargo:rustc-link-lib={}",
      fname
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .strip_prefix("lib")
        .unwrap()
    );
    println!(
      "cargo:rustc-link-search={}",
      p.parent()
        .expect("no library path in LIBXML2 env")
        .to_string_lossy()
    );
    None
  } else {    
    #[cfg(any(target_family = "unix", target_os = "macos"))]
    {
      let lib = pkg_config::Config::new()
        .probe("libxml-2.0")
        .expect("Couldn't find libxml2 via pkg-config");
      return Some(ProbedLib {
        include_paths: lib.include_paths,
        version: lib.version,
      })
    }

    #[cfg(windows)]
    {
      if vcpkg_dep::find() {
        return None
      }
    }
    
    panic!("Could not find libxml2.")
  }
}

fn generate_bindings(header_dirs: Vec<PathBuf>, output_path: &Path) {
  let bindings = bindgen::Builder::default()
    .header("src/wrapper.h")
    .opaque_type("max_align_t")
    // invalidate build as soon as the wrapper changes
    .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    .layout_tests(true)
    .clang_args(&["-DPKG-CONFIG"])
    .clang_args(
      header_dirs.iter()
        .map(|dir| format!("-I{}", dir.display()))
    );
  bindings
    .generate()
    .expect("failed to generate bindings with bindgen")
    .write_to_file(output_path)
    .expect("Failed to write bindings.rs");
}

fn main() {
  let bindings_path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("bindings.rs");
  // declare availability of config variable (without setting it)
  println!("cargo::rustc-check-cfg=cfg(libxml_older_than_2_12)");

  if let Some(probed_lib) = find_libxml2() {
    // if we could find header files, generate fresh bindings from them
    generate_bindings(probed_lib.include_paths, &bindings_path);
  } else {
    // otherwise, use the default bindings on platforms where pkg-config isn't available
    fs::copy(PathBuf::from("src/default_bindings.rs"), bindings_path)
      .expect("Failed to copy the default bindings to the build directory");
    // for now, assume that the library is older than 2.12, because that's what those bindings are computed with
    println!("cargo::rustc-cfg=libxml_older_than_2_12");
  }
}

#[cfg(target_family = "windows")]
mod vcpkg_dep {
  pub fn find() -> bool {
    if vcpkg::find_package("libxml2").is_ok() {
      return true;
    }
    false
  }
}
