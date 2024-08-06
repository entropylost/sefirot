use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-search=/usr/local/cuda/targets/x86_64-linux/lib/stubs");
    println!("cargo:rustc-link-lib=cuda");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
