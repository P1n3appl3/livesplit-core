use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// should be $WORKSPACE_ROOT/target/debug
fn out_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(env::var("OUT_DIR").ok()?)
            .parent()?
            .parent()?
            .to_path_buf(),
    )
}

// Compile all the wasm autosplitters so the tests can load them
fn main() {
    let splitter_dir = PathBuf::from("tests").join("splitters");
    let out_dir = out_dir().unwrap();

    println!("cargo:rerun-if-changed={}", splitter_dir.to_string_lossy());
    for path in fs::read_dir(splitter_dir).unwrap() {
        let path = path.unwrap().path();
        let path = path.to_string_lossy();
        let mut compile = Command::new("rustc")
            .arg("--crate-type=cdylib")
            .arg("--target=wasm32-unknown-unknown")
            .arg("--out-dir")
            .arg(&out_dir)
            .arg(&*path)
            .spawn()
            .expect("Failed to run compiler");
        assert!(compile.wait().unwrap().success());
    }
}
