fn main() {
  if let Ok(ref s) = std::env::var("LIBXML2") {
      // println!("{:?}", std::env::vars());
      // panic!("set libxml2.");
      let p = std::path::Path::new(s);
      let fname = std::path::Path::new(p.file_name().expect("no file name in LIBXML2 env"));
      assert!(p.is_file());
      println!("cargo:rustc-link-lib={}", fname.file_stem().unwrap().to_string_lossy());
      println!("cargo:rustc-link-search={}", p.parent().expect("no library path in LIBXML2 env").to_string_lossy());
  } else {
    #[cfg(any(target_family = "unix", target_os = "macos"))]
        {
          if pkg_config_dep::find() {
            return;
          }
        }

    #[cfg(windows)]
        {
          if vcpkg_dep::find() {
            return;
          }
        }

    panic!("Could not find libxml2.")
  }
}

#[cfg(any(target_family="unix", target_os="macos"))]
mod pkg_config_dep {
  pub fn find() -> bool {
    if pkg_config::find_library("libxml-2.0").is_ok() {
      return true;
    }
    false
  }
}

#[cfg(target_family="windows")]
mod vcpkg_dep {
  pub fn find() -> bool {
    if vcpkg::find_package("libxml2").is_ok() {
      return true
    }
    false
  }
}