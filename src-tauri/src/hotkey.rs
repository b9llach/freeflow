use parking_lot::Mutex;
use rdev::{listen, Event, EventType, Key};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::settings::HotkeyMode;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HotkeyEvent {
    Start,
    Stop,
}

#[derive(Clone)]
pub struct HotkeyState {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    key: Key,
    mode: HotkeyMode,
    pressed: bool,
    toggled_on: bool,
    enabled: bool,
}

impl HotkeyState {
    pub fn new(key: Key, mode: HotkeyMode) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                key,
                mode,
                pressed: false,
                toggled_on: false,
                enabled: true,
            })),
        }
    }

    pub fn set_key(&self, key: Key) {
        let mut g = self.inner.lock();
        g.key = key;
        g.pressed = false;
        g.toggled_on = false;
    }

    pub fn set_mode(&self, mode: HotkeyMode) {
        let mut g = self.inner.lock();
        g.mode = mode;
        g.pressed = false;
        g.toggled_on = false;
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.inner.lock().enabled = enabled;
    }
}

pub fn spawn_listener<F>(state: HotkeyState, on_event: F)
where
    F: Fn(HotkeyEvent) + Send + 'static,
{
    std::thread::spawn(move || {
        let cb = move |event: Event| {
            let (is_press, key) = match event.event_type {
                EventType::KeyPress(k) => (true, k),
                EventType::KeyRelease(k) => (false, k),
                _ => return,
            };
            let mut g = state.inner.lock();
            if !g.enabled || key != g.key {
                return;
            }
            match g.mode {
                HotkeyMode::PushToTalk => {
                    if is_press {
                        if !g.pressed {
                            g.pressed = true;
                            drop(g);
                            on_event(HotkeyEvent::Start);
                        }
                    } else if g.pressed {
                        g.pressed = false;
                        drop(g);
                        on_event(HotkeyEvent::Stop);
                    }
                }
                HotkeyMode::Toggle => {
                    if is_press && !g.pressed {
                        g.pressed = true;
                        let now_on = !g.toggled_on;
                        g.toggled_on = now_on;
                        drop(g);
                        on_event(if now_on {
                            HotkeyEvent::Start
                        } else {
                            HotkeyEvent::Stop
                        });
                    } else if !is_press {
                        g.pressed = false;
                    }
                }
            }
        };
        if let Err(e) = listen(cb) {
            tracing::error!(error = ?e, "rdev listen failed");
        }
    });
}

pub fn parse_key(name: &str) -> Option<Key> {
    let n = name.trim();
    Some(match n {
        "ControlLeft" | "LControl" | "LCtrl" => Key::ControlLeft,
        "ControlRight" | "RControl" | "RCtrl" => Key::ControlRight,
        "ShiftLeft" | "LShift" => Key::ShiftLeft,
        "ShiftRight" | "RShift" => Key::ShiftRight,
        "Alt" | "LAlt" | "AltLeft" => Key::Alt,
        "AltGr" | "RAlt" | "AltRight" => Key::AltGr,
        "MetaLeft" | "LMeta" | "LWin" | "Super" => Key::MetaLeft,
        "MetaRight" | "RMeta" | "RWin" => Key::MetaRight,
        "CapsLock" => Key::CapsLock,
        "Tab" => Key::Tab,
        "Space" => Key::Space,
        "Escape" => Key::Escape,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        other => {
            if other.len() == 1 {
                let ch = other.chars().next().unwrap().to_ascii_uppercase();
                match ch {
                    'A' => Key::KeyA, 'B' => Key::KeyB, 'C' => Key::KeyC, 'D' => Key::KeyD,
                    'E' => Key::KeyE, 'F' => Key::KeyF, 'G' => Key::KeyG, 'H' => Key::KeyH,
                    'I' => Key::KeyI, 'J' => Key::KeyJ, 'K' => Key::KeyK, 'L' => Key::KeyL,
                    'M' => Key::KeyM, 'N' => Key::KeyN, 'O' => Key::KeyO, 'P' => Key::KeyP,
                    'Q' => Key::KeyQ, 'R' => Key::KeyR, 'S' => Key::KeyS, 'T' => Key::KeyT,
                    'U' => Key::KeyU, 'V' => Key::KeyV, 'W' => Key::KeyW, 'X' => Key::KeyX,
                    'Y' => Key::KeyY, 'Z' => Key::KeyZ,
                    _ => return None,
                }
            } else {
                return None;
            }
        }
    })
}

pub fn key_to_name(key: Key) -> String {
    match key {
        Key::ControlLeft => "ControlLeft",
        Key::ControlRight => "ControlRight",
        Key::ShiftLeft => "ShiftLeft",
        Key::ShiftRight => "ShiftRight",
        Key::Alt => "Alt",
        Key::AltGr => "AltGr",
        Key::MetaLeft => "MetaLeft",
        Key::MetaRight => "MetaRight",
        Key::CapsLock => "CapsLock",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Escape => "Escape",
        other => return format!("{other:?}"),
    }
    .to_string()
}
