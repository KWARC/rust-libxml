//! We'll have to compile our helper_functions.c and create the
//! static library libhelper_functions.a
//! We'll assume that `gcc` and `ar` are installed and that the
//! header files are in `/usr/include/libxml2`.
//! In the future, we should move to a more flexible solution.


fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = |f : &str| format!("{}/{}", out_dir, f);
    std::process::Command::new("gcc")
        .args(&["src/helper_functions.c", "-I/usr/include/libxml2", 
                "-lxml", "-c", "-fPIC", "-o",
                &out_path("helper_functions.o")])
        .status().unwrap();
    std::process::Command::new("ar")
        .args(&["-crs", &out_path("/libhelper_functions.a"),
                &out_path("/helper_functions.o")])
        .status().unwrap();
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=helper_functions");
}

