#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

mod ffi;

// This test triggers a series of timer events based on how far it gets to
// reading a value from a loaded module of an attached process. The order of
// events is as follows:
//
// (attach) -> Start -> (find module) -> Split -> (read mem) -> Split -> (check
// read value) -> Split

#[no_mangle]
pub extern "C" fn configure() {
    let process_name = "fakegame";
    let module_name = "fakemodule";
    unsafe {
        let proc = ffi::attach(process_name.as_ptr() as u32, process_name.len() as u32);
        if proc == 0 {
            return;
        }
        ffi::start();
        let base_addr =
            ffi::get_module(proc, module_name.as_ptr() as u32, module_name.len() as u32);
        if base_addr == 0 {
            return;
        }
        ffi::split();
        let mut buf: u8 = 0xff;
        // TODO: make this cross platform
        // on linux the .rodata section is 2kb past the base (hopefully the compiler
        // stays consistent)
        if ffi::read_mem(proc, base_addr + 0x2000, &mut buf as *mut u8 as u32, 1) == 0 {
            return;
        }
        ffi::split();
        if buf == 42 {
            ffi::split()
        }
    }
}

#[no_mangle]
pub extern "C" fn update() {}
