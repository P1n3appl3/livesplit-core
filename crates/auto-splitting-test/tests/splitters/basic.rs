#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

mod ffi;

#[no_mangle]
pub extern "C" fn configure() {
    unsafe {
        ffi::start();
        ffi::split();
        ffi::pause_game_time();
        ffi::resume_game_time();
        ffi::reset();
    }
}

#[no_mangle]
pub extern "C" fn update() {}
