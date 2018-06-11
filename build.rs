extern crate pkg_config;
extern crate gcc;

fn main() {
  let pkginfo = match pkg_config::find_library("libxml-2.0") {
    Ok(pkginfo) => pkginfo,
    Err(e) => {
      println!("Couldn't find libxml: {}", e);
      std::process::exit(1);
    }
  };

  let mut gcc_builder = gcc::Build::new();
  gcc_builder.file("src/helper_functions.c");

  for include_path in pkginfo.include_paths.iter() {
    gcc_builder.include(include_path);
  }

  gcc_builder.compile("libhelper_functions.a");
}

