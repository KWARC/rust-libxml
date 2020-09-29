fn main() {
  #[cfg(any(unix, macos))]
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

#[cfg(any(unix, macos))]
mod pkg_config_dep {
  pub fn find() -> bool {
    if pkg_config::find_library("libxml-2.0").is_ok() {
      return true;
    }
    false
  }
}

#[cfg(windows)]
mod vcpkg_dep {
  pub fn find() -> bool {
    if vcpkg::find_package("libxml2").is_ok() {
      return true
    }
    false
  }
}