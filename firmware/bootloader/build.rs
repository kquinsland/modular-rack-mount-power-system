use std::{env, fs, path::PathBuf};

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    fs::copy("memory.x", output.join("memory.x")).expect("copy bootloader linker script");
    println!("cargo:rustc-link-search={}", output.display());
    println!("cargo:rerun-if-changed=memory.x");
}
