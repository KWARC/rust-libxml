fn main() {  
  if cfg!(not(feature = "pkg-config")) && cfg!(not(feature = "vcpkg"))
  {
    panic!(r#"Enable "pkg-config" or "vcpkg" feature flags to locate libxml2"#)
  }

  #[cfg(feature = "pkg-config")]
  {
    if pkg_config_dep::find() {
      return;
    }
  }

  #[cfg(feature = "vcpkg")]
  {
    if vcpkg_dep::find() {
      return;
    } 
  }

  panic!("Could not find libxml2.")
}

#[cfg(feature = "pkg-config")]
mod pkg_config_dep {
  use pkg_config;
  pub fn find() -> bool {
    if pkg_config::find_library("libxml-2.0").is_ok() {
      return true;
    }
    false
  }
}

#[cfg(feature = "vcpkg")]
mod vcpkg_dep {
  use vcpkg;
  pub fn find() -> bool {
    if vcpkg::find_package("libxml2").is_ok() {
      return true
    }
    false
  }
}