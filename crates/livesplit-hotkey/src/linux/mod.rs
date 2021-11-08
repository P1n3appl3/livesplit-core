use crate::KeyCode;
use input::{
    event::{
        keyboard::{KeyState, KeyboardEventTrait},
        KeyboardEvent::Key,
    },
    Event, Libinput, LibinputInterface,
};
use libc::{O_RDONLY, O_RDWR, O_WRONLY};
use std::fs::{File, OpenOptions};
use std::os::unix::{
    fs::OpenOptionsExt,
    io::{FromRawFd, IntoRawFd},
};
use std::path::Path;

use mio::{unix::SourceFd, Events, Interest, Poll, Token, Waker};
use promising_future::{future_promise, Promise};
use std::{
    collections::hash_map::HashMap,
    os::unix::prelude::{AsRawFd, RawFd},
    sync::mpsc::{channel, Sender},
    thread::{self, JoinHandle},
};

#[derive(Debug, Copy, Clone, snafu::Snafu)]
pub enum Error {
    EPoll,
    ThreadStopped,
    AlreadyRegistered,
    NotRegistered,
    UnknownKey,
    LibInput,
}

pub type Result<T> = std::result::Result<T, Error>;

enum Message {
    Register(
        KeyCode,
        Box<dyn FnMut() + Send + 'static>,
        Promise<Result<()>>,
    ),
    Unregister(KeyCode, Promise<Result<()>>),
    End,
}

const INPUT_TOKEN: Token = Token(0);
const PING_TOKEN: Token = Token(1);

pub struct Hook {
    sender: Sender<Message>,
    waker: Waker,
    join_handle: Option<JoinHandle<Result<()>>>,
}

impl Drop for Hook {
    fn drop(&mut self) {
        self.sender.send(Message::End).ok();
        self.waker.wake().ok();
        if let Some(handle) = self.join_handle.take() {
            handle.join().ok();
        }
    }
}

struct Interface;
impl LibinputInterface for Interface {
    fn open_restricted(
        &mut self,
        path: &Path,
        flags: i32,
    ) -> std::result::Result<RawFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read((flags & O_RDONLY != 0) | (flags & O_RDWR != 0))
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
            .open(path)
            .map(|file| file.into_raw_fd())
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: RawFd) {
        unsafe {
            File::from_raw_fd(fd);
        }
    }
}

impl Hook {
    pub fn new() -> Result<Self> {
        let (sender, receiver) = channel();
        let mut poll = Poll::new().map_err(|_| Error::EPoll)?;
        let waker = Waker::new(poll.registry(), PING_TOKEN).map_err(|_| Error::EPoll)?;

        let join_handle = thread::spawn(move || -> Result<()> {
            let mut result = Ok(());
            let mut events = Events::with_capacity(1024);
            let mut hotkeys: HashMap<u32, Box<dyn FnMut() + Send>> = HashMap::new();
            let mut input = Libinput::new_with_udev(Interface);

            input.udev_assign_seat("seat0").unwrap();
            poll.registry()
                .register(
                    &mut SourceFd(&input.as_raw_fd()),
                    INPUT_TOKEN,
                    Interest::READABLE,
                )
                .map_err(|_| Error::EPoll)?;

            'event_loop: loop {
                if poll.poll(&mut events, None).is_err() {
                    result = Err(Error::EPoll);
                    break 'event_loop;
                }

                for mio_event in &events {
                    if mio_event.token() == PING_TOKEN {
                        for message in receiver.try_iter() {
                            match message {
                                Message::Register(key, callback, promise) => {
                                    promise.set(code_for(key).and_then(|k| {
                                        if hotkeys.insert(k, callback).is_some() {
                                            Err(Error::AlreadyRegistered)
                                        } else {
                                            Ok(())
                                        }
                                    }))
                                }
                                Message::Unregister(key, promise) => promise.set(
                                    code_for(key)
                                        .and_then(|k| {
                                            hotkeys.remove(&k).ok_or(Error::NotRegistered)
                                        })
                                        .and(Ok(())),
                                ),
                                Message::End => {
                                    break 'event_loop;
                                }
                            }
                        }
                    } else if mio_event.token() == INPUT_TOKEN {
                        input.dispatch().unwrap();
                        for event in &mut input {
                            if let Event::Keyboard(Key(k)) = event {
                                if k.key_state() == KeyState::Pressed {
                                    println!("Key: {}", k.key());
                                    if let Some(callback) = hotkeys.get_mut(&k.key()) {
                                        callback();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            result
        });

        Ok(Hook {
            sender,
            waker,
            join_handle: Some(join_handle),
        })
    }

    pub fn register<F>(&self, hotkey: KeyCode, callback: F) -> Result<()>
    where
        F: FnMut() + Send + 'static,
    {
        let (future, promise) = future_promise();

        self.sender
            .send(Message::Register(hotkey, Box::new(callback), promise))
            .map_err(|_| Error::ThreadStopped)?;

        self.waker.wake().map_err(|_| Error::ThreadStopped)?;

        future.value().ok_or(Error::ThreadStopped)?
    }

    pub fn unregister(&self, hotkey: KeyCode) -> Result<()> {
        let (future, promise) = future_promise();

        self.sender
            .send(Message::Unregister(hotkey, promise))
            .map_err(|_| Error::ThreadStopped)?;

        self.waker.wake().map_err(|_| Error::ThreadStopped)?;

        future.value().ok_or(Error::ThreadStopped)?
    }
}

pub(crate) fn try_resolve(_key_code: KeyCode) -> Option<String> {
    None
}

fn code_for(key: KeyCode) -> Result<u32> {
    use KeyCode::*;
    Ok(match key {
        Escape => 0x0001,
        Digit1 => 0x0002,
        Digit2 => 0x0003,
        Digit3 => 0x0004,
        Digit4 => 0x0005,
        Digit5 => 0x0006,
        Digit6 => 0x0007,
        Digit7 => 0x0008,
        Digit8 => 0x0009,
        Digit9 => 0x000a,
        Digit0 => 0x000b,
        Minus => 0x000c,
        Equal => 0x000d,
        Backspace => 0x000e,
        Tab => 0x000f,
        KeyQ => 0x0010,
        KeyW => 0x0011,
        KeyE => 0x0012,
        KeyR => 0x0013,
        KeyT => 0x0014,
        KeyY => 0x0015,
        KeyU => 0x0016,
        KeyI => 0x0017,
        KeyO => 0x0018,
        KeyP => 0x0019,
        BracketLeft => 0x001a,
        BracketRight => 0x001b,
        Enter => 0x001c,
        ControlLeft => 0x001d,
        KeyA => 0x001e,
        KeyS => 0x001f,
        KeyD => 0x0020,
        KeyF => 0x0021,
        KeyG => 0x0022,
        KeyH => 0x0023,
        KeyJ => 0x0024,
        KeyK => 0x0025,
        KeyL => 0x0026,
        Semicolon => 0x0027,
        Quote => 0x0028,
        Backquote => 0x0029,
        ShiftLeft => 0x002a,
        Backslash => 0x002b,
        KeyZ => 0x002c,
        KeyX => 0x002d,
        KeyC => 0x002e,
        KeyV => 0x002f,
        KeyB => 0x0030,
        KeyN => 0x0031,
        KeyM => 0x0032,
        Comma => 0x0033,
        Period => 0x0034,
        Slash => 0x0035,
        ShiftRight => 0x0036,
        NumpadMultiply => 0x0037,
        AltLeft => 0x0038,
        Space => 0x0039,
        CapsLock => 0x003a,
        F1 => 0x003b,
        F2 => 0x003c,
        F3 => 0x003d,
        F4 => 0x003e,
        F5 => 0x003f,
        F6 => 0x0040,
        F7 => 0x0041,
        F8 => 0x0042,
        F9 => 0x0043,
        F10 => 0x0044,
        NumLock => 0x0045,
        ScrollLock => 0x0046,
        Numpad7 => 0x0047,
        Numpad8 => 0x0048,
        Numpad9 => 0x0049,
        NumpadSubtract => 0x004a,
        Numpad4 => 0x004b,
        Numpad5 => 0x004c,
        Numpad6 => 0x004d,
        NumpadAdd => 0x004e,
        Numpad1 => 0x004f,
        Numpad2 => 0x0050,
        Numpad3 => 0x0051,
        Numpad0 => 0x0052,
        NumpadDecimal => 0x0053,
        Lang5 => 0x0055, // Not Firefox, Not Safari
        IntlBackslash => 0x0056,
        F11 => 0x0057,
        F12 => 0x0058,
        IntlRo => 0x0059,
        Lang3 => 0x005a, // Not Firefox, Not Safari
        Lang4 => 0x005b, // Not Firefox, Not Safari
        Convert => 0x005c,
        KanaMode => 0x005d,
        NonConvert => 0x005e,
        NumpadEnter => 0x0060,
        ControlRight => 0x0061,
        NumpadDivide => 0x0062,
        PrintScreen => 0x0063,
        AltRight => 0x0064,
        Home => 0x0066,
        ArrowUp => 0x0067,
        PageUp => 0x0068,
        ArrowLeft => 0x0069,
        ArrowRight => 0x006a,
        End => 0x006b,
        ArrowDown => 0x006c,
        PageDown => 0x006d,
        Insert => 0x006e,
        Delete => 0x006f,
        AudioVolumeMute => 0x0071,
        AudioVolumeDown => 0x0072,
        AudioVolumeUp => 0x0073,
        Power => 0x0074, // Not Firefox, Not Safari
        NumpadEqual => 0x0075,
        Pause => 0x0077,
        ShowAllWindows => 0x0078, // Chrome only
        NumpadComma => 0x0079,
        Lang1 => 0x007a,
        Lang2 => 0x007b,
        IntlYen => 0x007c,
        MetaLeft => 0x007d,
        MetaRight => 0x007e,
        ContextMenu => 0x007f,
        BrowserStop => 0x0080,
        Again => 0x0081,
        Props => 0x0082, // Not Chrome
        Undo => 0x0083,
        Select => 0x0084,
        Copy => 0x0085,
        Open => 0x0086,
        Paste => 0x0087,
        Find => 0x0088,
        Cut => 0x0089,
        Help => 0x008a,
        LaunchApp2 => 0x008c,
        Sleep => 0x008e, // Not Firefox, Not Safari
        WakeUp => 0x008f,
        LaunchApp1 => 0x0090,
        LaunchMail => 0x009B,
        BrowserFavorites => 0x009C,
        BrowserBack => 0x009E,
        BrowserForward => 0x009F,
        Eject => 0x00A1,
        MediaTrackNext => 0x00A3,
        MediaPlayPause => 0x00A4,
        MediaTrackPrevious => 0x00A5,
        MediaStop => 0x00A6,
        MediaRecord => 0x00A7, // Chrome only
        MediaRewind => 0x00A8, // Chrome only
        MediaSelect => 0x00AB,
        BrowserHome => 0x00AC,
        BrowserRefresh => 0x00AD,
        NumpadParenLeft => 0x00B3,  // Not Firefox, Not Safari
        NumpadParenRight => 0x00B4, // Not Firefox, Not Safari
        F13 => 0x00B7,
        F14 => 0x00B8,
        F15 => 0x00B9,
        F16 => 0x00BA,
        F17 => 0x00BB,
        F18 => 0x00BC,
        F19 => 0x00BD,
        F20 => 0x00BE,
        F21 => 0x00BF,
        F22 => 0x00C0,
        F23 => 0x00C1,
        F24 => 0x00C2,
        MediaPause => 0x00C9,       // Chrome only
        MediaPlay => 0x00CF,        // Chrome only
        MediaFastForward => 0x00D0, // Chrome only
        BrowserSearch => 0x00D9,
        BrightnessDown => 0x00E0,       // Chrome only
        BrightnessUp => 0x00E1,         // Chrome only
        DisplayToggleIntExt => 0x00E3,  // Chrome only
        MailSend => 0x00E7,             // Chrome only
        MailReply => 0x00E8,            // Chrome only
        MailForward => 0x00E9,          // Chrome only
        ZoomToggle => 0x0174,           // Chrome only
        LaunchControlPanel => 0x0243,   // Chrome only
        SelectTask => 0x0244,           // Chrome only
        LaunchScreenSaver => 0x0245,    // Chrome only
        LaunchAssistant => 0x0247,      // Chrome only
        KeyboardLayoutSelect => 0x0248, // Chrome only
        PrivacyScreenToggle => 0x0279,
        NumpadBackspace => todo!(),
        NumpadClear => todo!(),
        NumpadClearEntry => todo!(),
        NumpadHash => todo!(),
        NumpadMemoryAdd => todo!(),
        NumpadMemoryClear => todo!(),
        NumpadMemoryRecall => todo!(),
        NumpadMemoryStore => todo!(),
        NumpadMemorySubtract => todo!(),
        NumpadStar => todo!(),
        Fn => todo!(),
        FnLock => todo!(),
        Gamepad0 => todo!(),
        Gamepad1 => todo!(),
        Gamepad2 => todo!(),
        Gamepad3 => todo!(),
        Gamepad4 => todo!(),
        Gamepad5 => todo!(),
        Gamepad6 => todo!(),
        Gamepad7 => todo!(),
        Gamepad8 => todo!(),
        Gamepad9 => todo!(),
        Gamepad10 => todo!(),
        Gamepad11 => todo!(),
        Gamepad12 => todo!(),
        Gamepad13 => todo!(),
        Gamepad14 => todo!(),
        Gamepad15 => todo!(),
        Gamepad16 => todo!(),
        Gamepad17 => todo!(),
        Gamepad18 => todo!(),
        Gamepad19 => todo!(), // Chrome only
    })
}

/*
impl TryFrom<KeyCode> for Key {
    type Error = Error;
    fn try_from(k: KeyCode) -> Result<Self> {
        use self::KeyCode::*;
        Ok(match k {
            Again => Key::KEY_AGAIN,
            AltLeft => Key::KEY_LEFTALT,
            AltRight => Key::KEY_RIGHTALT,
            ArrowDown => Key::KEY_DOWN,
            ArrowLeft => Key::KEY_LEFT,
            ArrowRight => Key::KEY_RIGHT,
            ArrowUp => Key::KEY_UP,
            AudioVolumeDown => Key::KEY_VOLUMEUP,
            AudioVolumeMute => Key::KEY_MUTE,
            AudioVolumeUp => Key::KEY_VOLUMEDOWN,
            Backquote => Key::KEY_GRAVE,
            Backslash => Key::KEY_BACKSLASH,
            Backspace => Key::KEY_BACKSPACE,
            BracketLeft => Key::KEY_LEFTBRACE,
            BracketRight => Key::KEY_RIGHTBRACE,
            BrightnessDown => Key::KEY_BRIGHTNESSDOWN,
            BrightnessUp => Key::KEY_BRIGHTNESSUP,
            BrowserBack => Key::KEY_BACK,
            BrowserFavorites => Key::KEY_FAVORITES,
            BrowserForward => Key::KEY_FORWARD,
            BrowserHome => Key::KEY_HOMEPAGE,
            BrowserRefresh => Key::KEY_REFRESH,
            BrowserSearch => Key::KEY_SEARCH,
            BrowserStop => Key::KEY_STOP,
            CapsLock => Key::KEY_CAPSLOCK,
            Comma => Key::KEY_COMMA,
            ContextMenu => Key::KEY_CONTEXT_MENU,
            ControlLeft => Key::KEY_LEFTCTRL,
            ControlRight => Key::KEY_RIGHTCTRL,
            Convert => Key::KEY_KATAKANA,
            Copy => Key::KEY_COPY,
            Cut => Key::KEY_CUT,
            Delete => Key::KEY_DELETE,
            Digit0 => Key::KEY_0,
            Digit1 => Key::KEY_1,
            Digit2 => Key::KEY_2,
            Digit3 => Key::KEY_3,
            Digit4 => Key::KEY_4,
            Digit5 => Key::KEY_5,
            Digit6 => Key::KEY_6,
            Digit7 => Key::KEY_7,
            Digit8 => Key::KEY_8,
            Digit9 => Key::KEY_9,
            DisplayToggleIntExt => Key::KEY_DISPLAYTOGGLE,
            Eject => Key::KEY_EJECTCD,
            End => Key::KEY_END,
            Enter => Key::KEY_ENTER,
            Equal => Key::KEY_EQUAL,
            Escape => Key::KEY_ESC,
            F1 => Key::KEY_F1,
            F2 => Key::KEY_F2,
            F3 => Key::KEY_F3,
            F4 => Key::KEY_F4,
            F5 => Key::KEY_F5,
            F6 => Key::KEY_F6,
            F7 => Key::KEY_F7,
            F8 => Key::KEY_F8,
            F9 => Key::KEY_F9,
            F10 => Key::KEY_F10,
            F11 => Key::KEY_F11,
            F12 => Key::KEY_F12,
            F13 => Key::KEY_F13,
            F14 => Key::KEY_F14,
            F15 => Key::KEY_F15,
            F16 => Key::KEY_F16,
            F17 => Key::KEY_F17,
            F18 => Key::KEY_F18,
            F19 => Key::KEY_F19,
            F20 => Key::KEY_F20,
            F21 => Key::KEY_F21,
            F22 => Key::KEY_F22,
            F23 => Key::KEY_F23,
            F24 => Key::KEY_F24,
            Find => Key::KEY_FIND,
            Fn => Key::KEY_FN,
            Gamepad0 => Key::BTN_SOUTH,
            Gamepad1 => Key::BTN_EAST,
            Gamepad2 => Key::BTN_C,
            Gamepad3 => Key::BTN_NORTH,
            Gamepad4 => Key::BTN_WEST,
            Gamepad5 => Key::BTN_Z,
            Gamepad6 => Key::BTN_TL,
            Gamepad7 => Key::BTN_TR,
            Gamepad8 => Key::BTN_TL2,
            Gamepad9 => Key::BTN_TR2,
            Gamepad10 => Key::BTN_SELECT,
            Gamepad11 => Key::BTN_START,
            Gamepad12 => Key::BTN_MODE,
            Gamepad13 => Key::BTN_THUMBL,
            Help => Key::KEY_HELP,
            Home => Key::KEY_HOME,
            Insert => Key::KEY_INSERT,
            IntlYen => Key::KEY_YEN,
            KanaMode => Key::KEY_KATAKANAHIRAGANA,
            KeyA => Key::KEY_A,
            KeyB => Key::KEY_B,
            KeyC => Key::KEY_C,
            KeyD => Key::KEY_D,
            KeyE => Key::KEY_E,
            KeyF => Key::KEY_F,
            KeyG => Key::KEY_G,
            KeyH => Key::KEY_H,
            KeyI => Key::KEY_I,
            KeyJ => Key::KEY_J,
            KeyK => Key::KEY_K,
            KeyL => Key::KEY_L,
            KeyM => Key::KEY_M,
            KeyN => Key::KEY_N,
            KeyO => Key::KEY_O,
            KeyP => Key::KEY_P,
            KeyQ => Key::KEY_Q,
            KeyR => Key::KEY_R,
            KeyS => Key::KEY_S,
            KeyT => Key::KEY_T,
            KeyU => Key::KEY_U,
            KeyV => Key::KEY_V,
            KeyW => Key::KEY_W,
            KeyX => Key::KEY_X,
            KeyY => Key::KEY_Y,
            KeyZ => Key::KEY_Z,
            KeyboardLayoutSelect => Key::KEY_KBD_LAYOUT_NEXT,
            Lang1 => Key::KEY_LANGUAGE,
            LaunchAssistant => Key::KEY_ASSISTANT,
            LaunchControlPanel => Key::KEY_CONTROLPANEL,
            LaunchMail => Key::KEY_MAIL,
            LaunchScreenSaver => Key::KEY_SCREENSAVER,
            MailForward => Key::KEY_FORWARDMAIL,
            MailReply => Key::KEY_REPLY,
            MailSend => Key::KEY_SEND,
            MediaFastForward => Key::KEY_FASTFORWARD,
            MediaPause => Key::KEY_PAUSE,
            MediaPlay => Key::KEY_PLAY,
            MediaPlayPause => Key::KEY_PLAYPAUSE,
            MediaRecord => Key::KEY_RECORD,
            MediaRewind => Key::KEY_REWIND,
            MediaSelect => Key::KEY_SELECT,
            MediaStop => Key::KEY_STOPCD,
            MediaTrackNext => Key::KEY_NEXTSONG,
            MediaTrackPrevious => Key::KEY_PREVIOUSSONG,
            MetaLeft => Key::KEY_LEFTMETA,
            MetaRight => Key::KEY_RIGHTMETA,
            Minus => Key::KEY_MINUS,
            NumLock => Key::KEY_NUMLOCK,
            Numpad0 => Key::KEY_NUMERIC_0,
            Numpad1 => Key::KEY_NUMERIC_1,
            Numpad2 => Key::KEY_NUMERIC_2,
            Numpad3 => Key::KEY_NUMERIC_3,
            Numpad4 => Key::KEY_NUMERIC_4,
            Numpad5 => Key::KEY_NUMERIC_5,
            Numpad6 => Key::KEY_NUMERIC_6,
            Numpad7 => Key::KEY_NUMERIC_7,
            Numpad8 => Key::KEY_NUMERIC_8,
            Numpad9 => Key::KEY_NUMERIC_9,
            NumpadAdd => Key::KEY_KPPLUS,
            NumpadComma => Key::KEY_KPCOMMA,
            NumpadDecimal => Key::KEY_KPDOT,
            NumpadDivide => Key::KEY_KPSLASH,
            NumpadEnter => Key::KEY_KPENTER,
            NumpadEqual => Key::KEY_KPEQUAL,
            NumpadHash => Key::KEY_NUMERIC_POUND,
            NumpadParenLeft => Key::KEY_KPLEFTPAREN,
            NumpadParenRight => Key::KEY_KPRIGHTPAREN,
            NumpadStar => Key::KEY_NUMERIC_STAR,
            NumpadSubtract => Key::KEY_KPMINUS,
            Open => Key::KEY_OPEN,
            PageDown => Key::KEY_PAGEDOWN,
            PageUp => Key::KEY_PAGEUP,
            Paste => Key::KEY_PASTE,
            Pause => Key::KEY_PAUSE,
            Period => Key::KEY_DOT,
            Power => Key::KEY_POWER,
            PrintScreen => Key::KEY_PRINT,
            PrivacyScreenToggle => Key::KEY_PRIVACY_SCREEN_TOGGLE,
            Props => Key::KEY_PROPS,
            Quote => Key::KEY_APOSTROPHE,
            ScrollLock => Key::KEY_SCROLLLOCK,
            Select => Key::KEY_SELECT,
            Semicolon => Key::KEY_SEMICOLON,
            ShiftLeft => Key::KEY_LEFTSHIFT,
            ShiftRight => Key::KEY_RIGHTSHIFT,
            ShowAllWindows => Key::KEY_CYCLEWINDOWS,
            Slash => Key::KEY_SLASH,
            Sleep => Key::KEY_SLEEP,
            Space => Key::KEY_SPACE,
            Tab => Key::KEY_TAB,
            Undo => Key::KEY_UNDO,
            WakeUp => Key::KEY_WAKEUP,
            ZoomToggle => Key::KEY_ZOOM,
            // TODO: test on a gamepad with 20 buttons to see what the higher indices are
            Gamepad14 | Gamepad15 | Gamepad16 | Gamepad17 | Gamepad18 | Gamepad19 => {
                return Err(Error::UnknownKey)
            }
            // TODO: find a keyboard with these keys and see which they correspond to
            SelectTask | IntlBackslash | IntlRo | Lang2 | Lang3 | Lang4 | Lang5
            | NonConvert | FnLock | LaunchApp1 | LaunchApp2 | NumpadBackspace
            | NumpadClear | NumpadClearEntry | NumpadMemoryAdd | NumpadMemoryClear
            | NumpadMemoryRecall | NumpadMemoryStore | NumpadMemorySubtract
            | NumpadMultiply => return Err(Error::UnknownKey),
        })
    }
}
*/
