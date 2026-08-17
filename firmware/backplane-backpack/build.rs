use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=memory.x");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"));
    fs::copy("memory.x", output.join("memory.x")).expect("copy checked-in memory.x");
    println!("cargo:rustc-link-search={}", output.display());
}
