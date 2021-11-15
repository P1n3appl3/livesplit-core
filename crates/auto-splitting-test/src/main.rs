use libloading::{Library, Symbol};
use std::{env, thread, time::Duration};

fn load_lib(name: &str) -> Option<Library> {
    let dylib = libloading::library_filename(name);
    let path = env::current_dir()
        .ok()?
        .parent()?
        .parent()?
        .join("target")
        .join("debug")
        .join(dylib);
    unsafe { Library::new(path) }.ok()
}

fn main() {
    let lib = load_lib("fakemodule").unwrap();
    let magic: Symbol<*const u8> = unsafe { lib.get(b"magic\0") }.unwrap();
    println!(
        "Found magic value at: {:#X} with value: {}",
        *magic as u64,
        unsafe { **magic }
    );
    println!("Now sitting around and pretending to run a game...");
    // this should be long enough for any single test to run
    thread::sleep(Duration::from_secs(10));
}
