#[allow(unused)]
extern "C" {
    pub(crate) fn print_message(ptr: *const u8, len: usize);
    pub(crate) fn attach(ptr: u32, len: u32) -> u64;
    pub(crate) fn detach(handle: u64);
    pub(crate) fn get_module(handle: u64, ptr: u32, len: u32) -> u64;
    pub(crate) fn read_mem(handle: u64, address: u64, buf: u32, buf_len: u32) -> u32;
    pub(crate) fn start();
    pub(crate) fn split();
    pub(crate) fn reset();
    pub(crate) fn set_tick_rate(rate: f64);
    pub(crate) fn set_variable(key: u32, key_len: u32, value: u32, value_len: u32);
    pub(crate) fn pause_game_time();
    pub(crate) fn resume_game_time();
    pub(crate) fn set_game_time(time: f64);
    pub(crate) fn get_timer_state() -> u32;
}
