#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// The runtime doesn't provide this function
extern "C" {
    pub fn not_a_function();
}

#[no_mangle]
pub extern "C" fn configure() {
    unsafe { not_a_function() }
}

#[no_mangle]
pub extern "C" fn update() {}
