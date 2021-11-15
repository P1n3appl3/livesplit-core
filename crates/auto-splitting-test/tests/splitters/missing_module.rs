#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

mod ffi;

// This test should load the binary correctly but then fail to find the module.
// So it should start, but not split

#[no_mangle]
pub extern "C" fn configure() {
    let process_name = "fakegame";
    let module_name = "WRONGMODULE";
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
    }
}

#[no_mangle]
pub extern "C" fn update() {}
