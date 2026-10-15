use super::JoypadButton;
use sdl2::keyboard::Keycode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyBinding {
    Single(String),
    Multiple(Vec<String>),
}

impl KeyBinding {
    pub fn keys(&self) -> Vec<&str> {
        match self {
            KeyBinding::Single(s) => vec![s.as_str()],
            KeyBinding::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerControlsConfig {
    #[serde(default)]
    pub up: Option<KeyBinding>,
    #[serde(default)]
    pub down: Option<KeyBinding>,
    #[serde(default)]
    pub left: Option<KeyBinding>,
    #[serde(default)]
    pub right: Option<KeyBinding>,
    #[serde(default)]
    pub a: Option<KeyBinding>,
    #[serde(default)]
    pub b: Option<KeyBinding>,
    #[serde(default)]
    pub select: Option<KeyBinding>,
    #[serde(default)]
    pub start: Option<KeyBinding>,
}

impl Default for PlayerControlsConfig {
    fn default() -> Self {
        Self::player1_default()
    }
}

impl PlayerControlsConfig {
    pub fn player1_default() -> Self {
        Self {
            up: Some(KeyBinding::Multiple(vec!["Up".into(), "W".into()])),
            down: Some(KeyBinding::Multiple(vec!["Down".into(), "S".into()])),
            left: Some(KeyBinding::Multiple(vec!["Left".into(), "A".into()])),
            right: Some(KeyBinding::Multiple(vec!["Right".into(), "D".into()])),
            a: Some(KeyBinding::Multiple(vec!["Z".into(), "K".into()])),
            b: Some(KeyBinding::Multiple(vec!["X".into(), "J".into()])),
            select: Some(KeyBinding::Multiple(vec!["Space".into(), "RShift".into()])),
            start: Some(KeyBinding::Single("Return".into())),
        }
    }

    pub fn player2_default() -> Self {
        Self {
            up: Some(KeyBinding::Single("I".into())),
            down: Some(KeyBinding::Single("K".into())),
            left: Some(KeyBinding::Single("J".into())),
            right: Some(KeyBinding::Single("L".into())),
            a: Some(KeyBinding::Single("O".into())),
            b: Some(KeyBinding::Single("U".into())),
            select: Some(KeyBinding::Single("Y".into())),
            start: Some(KeyBinding::Single("H".into())),
        }
    }

    pub fn empty() -> Self {
        Self {
            up: None,
            down: None,
            left: None,
            right: None,
            a: None,
            b: None,
            select: None,
            start: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.up.is_none()
            && self.down.is_none()
            && self.left.is_none()
            && self.right.is_none()
            && self.a.is_none()
            && self.b.is_none()
            && self.select.is_none()
            && self.start.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeysConfig {
    #[serde(default)]
    pub pause: Option<KeyBinding>,
    #[serde(default)]
    pub reset: Option<KeyBinding>,
    #[serde(default)]
    pub fast_forward: Option<KeyBinding>,
    #[serde(default)]
    pub mute: Option<KeyBinding>,
    #[serde(default)]
    pub audio_diagnostics: Option<KeyBinding>,
    #[serde(default)]
    pub audio_dump: Option<KeyBinding>,
    #[serde(default)]
    pub screenshot: Option<KeyBinding>,
    #[serde(default)]
    pub fullscreen: Option<KeyBinding>,
    #[serde(default)]
    pub exit: Option<KeyBinding>,
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            pause: Some(KeyBinding::Single("P".into())),
            reset: Some(KeyBinding::Single("R".into())),
            fast_forward: Some(KeyBinding::Single("Tab".into())),
            mute: Some(KeyBinding::Single("M".into())),
            audio_diagnostics: Some(KeyBinding::Single("F3".into())),
            audio_dump: Some(KeyBinding::Single("F4".into())),
            screenshot: Some(KeyBinding::Multiple(vec!["F12".into(), "F2".into()])),
            fullscreen: Some(KeyBinding::Single("F11".into())),
            exit: Some(KeyBinding::Single("Escape".into())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlsConfig {
    #[serde(default = "PlayerControlsConfig::player1_default")]
    pub player1: PlayerControlsConfig,
    #[serde(default = "default_player2")]
    pub player2: PlayerControlsConfig,
    #[serde(default)]
    pub hotkeys: HotkeysConfig,
}

fn default_player2() -> PlayerControlsConfig {
    PlayerControlsConfig::empty()
}

impl Default for ControlsConfig {
    fn default() -> Self {
        Self {
            player1: PlayerControlsConfig::player1_default(),
            player2: PlayerControlsConfig::player2_default(),
            hotkeys: HotkeysConfig::default(),
        }
    }
}

impl ControlsConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let p = path.as_ref();
        let content = fs::read_to_string(p).map_err(|e| {
            format!(
                "Failed to read controls configuration file '{}': {}",
                p.display(),
                e
            )
        })?;
        Self::from_json_str(&content).map_err(|e| {
            format!(
                "Failed to parse controls configuration from '{}': {}",
                p.display(),
                e
            )
        })
    }

    pub fn from_json_str(json_str: &str) -> Result<Self, String> {
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let p = path.as_ref();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize controls configuration: {}", e))?;
        fs::write(p, json).map_err(|e| {
            format!(
                "Failed to write controls configuration to '{}': {}",
                p.display(),
                e
            )
        })
    }

    pub fn load_or_default<P: AsRef<Path>>(path: Option<P>) -> (Self, Option<String>) {
        if let Some(p) = path {
            let path_ref = p.as_ref();
            match Self::from_file(path_ref) {
                Ok(cfg) => (cfg, Some(path_ref.display().to_string())),
                Err(e) => {
                    eprintln!("Warning: {}, using default controls", e);
                    (Self::default(), None)
                }
            }
        } else {
            let default_path = Path::new("controls.json");
            if default_path.exists() {
                match Self::from_file(default_path) {
                    Ok(cfg) => (cfg, Some("controls.json".to_string())),
                    Err(e) => {
                        eprintln!("Warning: {}, using default controls", e);
                        (Self::default(), None)
                    }
                }
            } else {
                (Self::default(), None)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputAction {
    Player1(JoypadButton),
    Player2(JoypadButton),
    Hotkey(HotkeyAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    Pause,
    Reset,
    FastForward,
    Mute,
    AudioDiagnostics,
    AudioDump,
    Screenshot,
    Fullscreen,
    Exit,
}

#[derive(Debug, Clone, Default)]
pub struct InputMap {
    map: HashMap<Keycode, Vec<InputAction>>,
}

impl InputMap {
    pub fn new(config: &ControlsConfig) -> Self {
        let mut input_map = Self::default();

        let mut bind = |binding: &Option<KeyBinding>, action: InputAction, is_p2: bool| {
            if let Some(b) = binding {
                for key_name in b.keys() {
                    if let Some(keycode) = parse_keycode(key_name) {
                        if let Some(existing) = input_map.map.get(&keycode) {
                            if is_p2
                                && existing.iter().any(|a| {
                                    matches!(a, InputAction::Player1(_) | InputAction::Hotkey(_))
                                })
                            {
                                eprintln!(
                                    "Warning: Key '{}' for Player 2 conflicts with existing binding, ignoring for Player 2",
                                    key_name
                                );
                                continue;
                            }
                            if !is_p2
                                && matches!(action, InputAction::Player1(_))
                                && existing.iter().any(|a| matches!(a, InputAction::Hotkey(_)))
                            {
                                eprintln!(
                                    "Warning: Key '{}' for Player 1 conflicts with Hotkey binding, ignoring for Player 1",
                                    key_name
                                );
                                continue;
                            }
                        }
                        input_map.map.entry(keycode).or_default().push(action);
                    } else {
                        eprintln!(
                            "Warning: Unknown key '{}' in controls configuration",
                            key_name
                        );
                    }
                }
            }
        };

        // Hotkeys
        bind(
            &config.hotkeys.pause,
            InputAction::Hotkey(HotkeyAction::Pause),
            false,
        );
        bind(
            &config.hotkeys.reset,
            InputAction::Hotkey(HotkeyAction::Reset),
            false,
        );
        bind(
            &config.hotkeys.fast_forward,
            InputAction::Hotkey(HotkeyAction::FastForward),
            false,
        );
        bind(
            &config.hotkeys.mute,
            InputAction::Hotkey(HotkeyAction::Mute),
            false,
        );
        bind(
            &config.hotkeys.audio_diagnostics,
            InputAction::Hotkey(HotkeyAction::AudioDiagnostics),
            false,
        );
        bind(
            &config.hotkeys.audio_dump,
            InputAction::Hotkey(HotkeyAction::AudioDump),
            false,
        );
        bind(
            &config.hotkeys.screenshot,
            InputAction::Hotkey(HotkeyAction::Screenshot),
            false,
        );
        bind(
            &config.hotkeys.fullscreen,
            InputAction::Hotkey(HotkeyAction::Fullscreen),
            false,
        );
        bind(
            &config.hotkeys.exit,
            InputAction::Hotkey(HotkeyAction::Exit),
            false,
        );

        // Player 1
        bind(
            &config.player1.up,
            InputAction::Player1(JoypadButton::Up),
            false,
        );
        bind(
            &config.player1.down,
            InputAction::Player1(JoypadButton::Down),
            false,
        );
        bind(
            &config.player1.left,
            InputAction::Player1(JoypadButton::Left),
            false,
        );
        bind(
            &config.player1.right,
            InputAction::Player1(JoypadButton::Right),
            false,
        );
        bind(
            &config.player1.a,
            InputAction::Player1(JoypadButton::A),
            false,
        );
        bind(
            &config.player1.b,
            InputAction::Player1(JoypadButton::B),
            false,
        );
        bind(
            &config.player1.select,
            InputAction::Player1(JoypadButton::Select),
            false,
        );
        bind(
            &config.player1.start,
            InputAction::Player1(JoypadButton::Start),
            false,
        );

        // Player 2
        bind(
            &config.player2.up,
            InputAction::Player2(JoypadButton::Up),
            true,
        );
        bind(
            &config.player2.down,
            InputAction::Player2(JoypadButton::Down),
            true,
        );
        bind(
            &config.player2.left,
            InputAction::Player2(JoypadButton::Left),
            true,
        );
        bind(
            &config.player2.right,
            InputAction::Player2(JoypadButton::Right),
            true,
        );
        bind(
            &config.player2.a,
            InputAction::Player2(JoypadButton::A),
            true,
        );
        bind(
            &config.player2.b,
            InputAction::Player2(JoypadButton::B),
            true,
        );
        bind(
            &config.player2.select,
            InputAction::Player2(JoypadButton::Select),
            true,
        );
        bind(
            &config.player2.start,
            InputAction::Player2(JoypadButton::Start),
            true,
        );

        input_map
    }

    pub fn get_actions(&self, key: &Keycode) -> Option<&[InputAction]> {
        self.map.get(key).map(|v| v.as_slice())
    }
}

pub fn parse_keycode(s: &str) -> Option<Keycode> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Check direct SDL2 key name lookup first
    if let Some(k) = Keycode::from_name(trimmed) {
        return Some(k);
    }

    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "enter" | "return" => Some(Keycode::Return),
        "esc" | "escape" => Some(Keycode::Escape),
        "space" => Some(Keycode::Space),
        "tab" => Some(Keycode::Tab),
        "backspace" => Some(Keycode::Backspace),
        "delete" => Some(Keycode::Delete),
        "insert" => Some(Keycode::Insert),
        "home" => Some(Keycode::Home),
        "end" => Some(Keycode::End),
        "pageup" | "page_up" => Some(Keycode::PageUp),
        "pagedown" | "page_down" => Some(Keycode::PageDown),
        "up" | "arrowup" => Some(Keycode::Up),
        "down" | "arrowdown" => Some(Keycode::Down),
        "left" | "arrowleft" => Some(Keycode::Left),
        "right" | "arrowright" => Some(Keycode::Right),
        "lshift" | "leftshift" | "shift" => Some(Keycode::LShift),
        "rshift" | "rightshift" => Some(Keycode::RShift),
        "lctrl" | "leftctrl" | "ctrl" | "control" => Some(Keycode::LCtrl),
        "rctrl" | "rightctrl" => Some(Keycode::RCtrl),
        "lalt" | "leftalt" | "alt" => Some(Keycode::LAlt),
        "ralt" | "rightalt" => Some(Keycode::RAlt),
        // Letter keys 'a'..='z'
        "a" => Some(Keycode::A),
        "b" => Some(Keycode::B),
        "c" => Some(Keycode::C),
        "d" => Some(Keycode::D),
        "e" => Some(Keycode::E),
        "f" => Some(Keycode::F),
        "g" => Some(Keycode::G),
        "h" => Some(Keycode::H),
        "i" => Some(Keycode::I),
        "j" => Some(Keycode::J),
        "k" => Some(Keycode::K),
        "l" => Some(Keycode::L),
        "m" => Some(Keycode::M),
        "n" => Some(Keycode::N),
        "o" => Some(Keycode::O),
        "p" => Some(Keycode::P),
        "q" => Some(Keycode::Q),
        "r" => Some(Keycode::R),
        "s" => Some(Keycode::S),
        "t" => Some(Keycode::T),
        "u" => Some(Keycode::U),
        "v" => Some(Keycode::V),
        "w" => Some(Keycode::W),
        "x" => Some(Keycode::X),
        "y" => Some(Keycode::Y),
        "z" => Some(Keycode::Z),
        // Number keys '0'..='9'
        "0" => Some(Keycode::Num0),
        "1" => Some(Keycode::Num1),
        "2" => Some(Keycode::Num2),
        "3" => Some(Keycode::Num3),
        "4" => Some(Keycode::Num4),
        "5" => Some(Keycode::Num5),
        "6" => Some(Keycode::Num6),
        "7" => Some(Keycode::Num7),
        "8" => Some(Keycode::Num8),
        "9" => Some(Keycode::Num9),
        // Keypad numbers
        "kp_0" | "kp0" | "numpad0" | "numpad_0" | "num0" => Some(Keycode::Kp0),
        "kp_1" | "kp1" | "numpad1" | "numpad_1" | "num1" => Some(Keycode::Kp1),
        "kp_2" | "kp2" | "numpad2" | "numpad_2" | "num2" => Some(Keycode::Kp2),
        "kp_3" | "kp3" | "numpad3" | "numpad_3" | "num3" => Some(Keycode::Kp3),
        "kp_4" | "kp4" | "numpad4" | "numpad_4" | "num4" => Some(Keycode::Kp4),
        "kp_5" | "kp5" | "numpad5" | "numpad_5" | "num5" => Some(Keycode::Kp5),
        "kp_6" | "kp6" | "numpad6" | "numpad_6" | "num6" => Some(Keycode::Kp6),
        "kp_7" | "kp7" | "numpad7" | "numpad_7" | "num7" => Some(Keycode::Kp7),
        "kp_8" | "kp8" | "numpad8" | "numpad_8" | "num8" => Some(Keycode::Kp8),
        "kp_9" | "kp9" | "numpad9" | "numpad_9" | "num9" => Some(Keycode::Kp9),
        // Punctuation and symbols
        "." | "period" | "dot" => Some(Keycode::Period),
        "," | "comma" => Some(Keycode::Comma),
        "-" | "minus" => Some(Keycode::Minus),
        "=" | "equals" | "equal" => Some(Keycode::Equals),
        "+" | "plus" => Some(Keycode::Plus),
        ";" | "semicolon" => Some(Keycode::Semicolon),
        "/" | "slash" => Some(Keycode::Slash),
        "\\" | "backslash" => Some(Keycode::Backslash),
        "[" | "leftbracket" | "lbracket" => Some(Keycode::LeftBracket),
        "]" | "rightbracket" | "rbracket" => Some(Keycode::RightBracket),
        "'" | "quote" => Some(Keycode::Quote),
        "`" | "backquote" | "grave" => Some(Keycode::Backquote),
        // Function keys F1..=F12
        "f1" => Some(Keycode::F1),
        "f2" => Some(Keycode::F2),
        "f3" => Some(Keycode::F3),
        "f4" => Some(Keycode::F4),
        "f5" => Some(Keycode::F5),
        "f6" => Some(Keycode::F6),
        "f7" => Some(Keycode::F7),
        "f8" => Some(Keycode::F8),
        "f9" => Some(Keycode::F9),
        "f10" => Some(Keycode::F10),
        "f11" => Some(Keycode::F11),
        "f12" => Some(Keycode::F12),
        _ => None,
    }
}
