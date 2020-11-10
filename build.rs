fn main() {
  #[cfg(any(target_family="unix", target_os="macos"))]
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