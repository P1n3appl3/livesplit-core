#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

mod ffi;

// after about a second, this splitter should double its tick rate

const RATE_A: f64 = 50.0;
const RATE_B: f64 = 100.0;

#[no_mangle]
pub extern "C" fn configure() {
    unsafe {
        ffi::set_tick_rate(RATE_A);
    }
}

static mut count: u32 = 0;

#[no_mangle]
pub extern "C" fn update() {
    unsafe {
        if count == RATE_A as u32 {
            ffi::set_tick_rate(RATE_B);
        }
        count += 1;
        ffi::split();
    }
}
