use std::{
    env,
    process::{Child, Command},
    sync::{Arc, RwLock},
    thread,
};

use livesplit_auto_splitting::{Error, Result, Runtime, Timer, TimerState};
use time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Start,
    Split,
    Reset,
    Pause,
    Resume,
    SetTime(Duration),
    SetVar(String, String),
}

#[test]
fn basic() {
    let timer = DummyTimer::default();
    let mut runtime = load_autosplitter("basic", timer.clone()).unwrap();
    runtime.step().unwrap();
    use Event::*;
    assert_eq!(
        timer.0.write().unwrap().history,
        [Start, Split, Pause, Resume, Reset]
    );
}

#[test]
fn expose_wrong_interface() {
    assert_eq!(
        load_autosplitter("wrong_interface", DummyTimer::default()).err(),
        Some(Error::InvalidInterface)
    )
}

#[test]
fn unknown_host_function() {
    assert!(matches!(
        load_autosplitter("unknown_func", DummyTimer::default()),
        Err(Error::WasmTimeEngine { err: _ })
    ))
}

#[test]
fn rough_tick_rate() {
    let timer = DummyTimer::default();
    let mut runtime = load_autosplitter("tick_rate", timer.clone()).unwrap();
    runtime.step().unwrap(); // configure
    let start = std::time::Instant::now();
    while std::time::Instant::now() - start < std::time::Duration::from_secs(1) {
        runtime.sleep();
        runtime.step().unwrap();
    }
    let first_half = timer.0.write().unwrap().history.len();
    let start = std::time::Instant::now();
    while std::time::Instant::now() - start < std::time::Duration::from_secs(1) {
        runtime.sleep();
        runtime.step().unwrap();
    }
    let total = timer.0.write().unwrap().history.len();
    // since the rate doubled after the first half, there should be ~3x more events
    let margin = 0.2;
    assert!(total < (first_half as f32 * (3.0 + margin)) as usize);
    assert!(total > (first_half as f32 * (3.0 - margin)) as usize);
}

// TODO: see tests/splitters/process_read.rs for why this is currently platform
// specific
#[cfg(unix)]
mod linux_tests {
    use super::*;
    use serial_test::serial;
    // The process tests have to be run serially so they don't interfere with one
    // another. an alternative would be making each one run on a different binary.
    #[test]
    #[serial]
    fn process_reading() {
        let mut game = launch_game("fakegame").unwrap();
        let timer = DummyTimer::default();
        let mut runtime = load_autosplitter("process_read", timer.clone()).unwrap();
        runtime.step().unwrap();
        game.kill().unwrap();
        use Event::*;
        assert_eq!(
            timer.0.write().unwrap().history,
            [Start, Split, Split, Split]
        );
    }

    #[test]
    #[serial]
    fn missing_process() {
        // the other processes were still showing up if this ran too soon after them
        let timer = DummyTimer::default();
        let mut runtime = load_autosplitter("missing_process", timer.clone()).unwrap();
        runtime.step().unwrap();
        // Since the game isn't running, the attach failed and it shouldn't have started
        assert_eq!(timer.0.write().unwrap().history, []);
    }

    #[test]
    #[serial]
    fn missing_module() {
        let timer = DummyTimer::default();
        let mut game = launch_game("fakegame").unwrap();
        let mut runtime = load_autosplitter("missing_module", timer.clone()).unwrap();
        runtime.step().unwrap();
        game.kill().unwrap();
        assert_eq!(timer.0.write().unwrap().history, [Event::Start]);
    }
}

// TODO: add tests for setting vars and gametime, as well as checking timer
// state

#[derive(Default, Debug)]
struct DummyInner {
    pub history: Vec<Event>,
    pub state: TimerState,
}

#[derive(Default, Clone, Debug)]
struct DummyTimer(Arc<RwLock<DummyInner>>);

impl Timer for DummyTimer {
    fn state(&self) -> TimerState {
        self.0.write().unwrap().state
    }
    fn start(&mut self) {
        self.0.write().unwrap().history.push(Event::Start)
    }
    fn split(&mut self) {
        self.0.write().unwrap().history.push(Event::Split)
    }
    fn reset(&mut self) {
        self.0.write().unwrap().history.push(Event::Reset)
    }
    fn pause_game_time(&mut self) {
        self.0.write().unwrap().history.push(Event::Pause)
    }
    fn resume_game_time(&mut self) {
        self.0.write().unwrap().history.push(Event::Resume)
    }
    fn set_game_time(&mut self, time: Duration) {
        self.0.write().unwrap().history.push(Event::SetTime(time))
    }
    fn set_variable(&mut self, key: &str, value: &str) {
        self.0
            .write()
            .unwrap()
            .history
            .push(Event::SetVar(key.to_string(), value.to_string()))
    }
}

fn load_autosplitter(name: &str, timer: DummyTimer) -> Result<Runtime<DummyTimer>> {
    let mut wasm_file = env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("debug")
        .join("build")
        .join(name);
    assert!(wasm_file.set_extension("wasm"));
    Runtime::new(wasm_file, timer)
}

fn launch_game(name: &str) -> Option<Child> {
    let path = env::current_dir()
        .ok()?
        .parent()?
        .parent()?
        .join("target")
        .join("debug")
        .join(name);
    let mut game = Command::new(path).spawn().ok()?;
    thread::sleep(std::time::Duration::from_millis(1000));
    // hopefully by now the process has loaded the dylib and not crashed
    assert!(matches!(game.try_wait(), Ok(None)));
    Some(game)
}
