use nes::controller::{
    config::parse_keycode, ControlsConfig, HotkeyAction, InputAction, InputMap, JoypadButton,
};
use sdl2::keyboard::Keycode;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_default_controls_config() {
    let config = ControlsConfig::default();

    // Player 1 defaults
    assert!(config.player1.up.is_some());
    assert!(config.player1.down.is_some());
    assert!(config.player1.left.is_some());
    assert!(config.player1.right.is_some());
    assert!(config.player1.a.is_some());
    assert!(config.player1.b.is_some());
    assert!(config.player1.select.is_some());
    assert!(config.player1.start.is_some());

    // Verify key binding accessor
    let up_keys = config.player1.up.as_ref().unwrap().keys();
    assert!(up_keys.contains(&"Up"));
    assert!(up_keys.contains(&"W"));

    let start_keys = config.player1.start.as_ref().unwrap().keys();
    assert_eq!(start_keys, vec!["Return"]);

    // Player 2 defaults
    let p2_up = config.player2.up.as_ref().unwrap().keys();
    assert_eq!(p2_up, vec!["I"]);
    let p2_a = config.player2.a.as_ref().unwrap().keys();
    assert_eq!(p2_a, vec!["O"]);

    // Hotkey defaults
    let pause_keys = config.hotkeys.pause.as_ref().unwrap().keys();
    assert_eq!(pause_keys, vec!["P"]);
    let shot_keys = config.hotkeys.screenshot.as_ref().unwrap().keys();
    assert!(shot_keys.contains(&"F12"));
    assert!(shot_keys.contains(&"F2"));
}

#[test]
fn test_parse_keycode_cases_and_aliases() {
    // Basic letters and case insensitivity
    assert_eq!(parse_keycode("w"), Some(Keycode::W));
    assert_eq!(parse_keycode("W"), Some(Keycode::W));
    assert_eq!(parse_keycode("z"), Some(Keycode::Z));
    assert_eq!(parse_keycode("Z"), Some(Keycode::Z));

    // Numbers
    assert_eq!(parse_keycode("1"), Some(Keycode::Num1));
    assert_eq!(parse_keycode("9"), Some(Keycode::Num9));

    // Common aliases
    assert_eq!(parse_keycode("enter"), Some(Keycode::Return));
    assert_eq!(parse_keycode("ENTER"), Some(Keycode::Return));
    assert_eq!(parse_keycode("return"), Some(Keycode::Return));
    assert_eq!(parse_keycode("esc"), Some(Keycode::Escape));
    assert_eq!(parse_keycode("escape"), Some(Keycode::Escape));
    assert_eq!(parse_keycode("space"), Some(Keycode::Space));
    assert_eq!(parse_keycode("rshift"), Some(Keycode::RShift));
    assert_eq!(parse_keycode("lshift"), Some(Keycode::LShift));
    assert_eq!(parse_keycode("shift"), Some(Keycode::LShift));
    assert_eq!(parse_keycode("ctrl"), Some(Keycode::LCtrl));
    assert_eq!(parse_keycode("tab"), Some(Keycode::Tab));

    // Arrow keys
    assert_eq!(parse_keycode("up"), Some(Keycode::Up));
    assert_eq!(parse_keycode("arrowup"), Some(Keycode::Up));
    assert_eq!(parse_keycode("Down"), Some(Keycode::Down));
    assert_eq!(parse_keycode("Left"), Some(Keycode::Left));
    assert_eq!(parse_keycode("Right"), Some(Keycode::Right));

    // Function keys
    assert_eq!(parse_keycode("f1"), Some(Keycode::F1));
    assert_eq!(parse_keycode("F2"), Some(Keycode::F2));
    assert_eq!(parse_keycode("f12"), Some(Keycode::F12));

    // Unknown or empty keys
    assert_eq!(parse_keycode(""), None);
    assert_eq!(parse_keycode("   "), None);
    assert_eq!(parse_keycode("NonExistentKey999"), None);
}

#[test]
fn test_json_deserialization_custom_controls() {
    let custom_json = r#"{
        "player1": {
            "up": "W",
            "down": "S",
            "left": "A",
            "right": "D",
            "a": "J",
            "b": "K",
            "select": "U",
            "start": "I"
        },
        "player2": {
            "up": ["ArrowUp", "Num8"],
            "down": ["ArrowDown", "Num2"],
            "left": ["ArrowLeft", "Num4"],
            "right": ["ArrowRight", "Num6"],
            "a": "Num0",
            "b": "Period",
            "select": "Minus",
            "start": "Plus"
        },
        "hotkeys": {
            "pause": "Space",
            "reset": "Backspace",
            "fullscreen": "F",
            "exit": "Q"
        }
    }"#;

    let config = ControlsConfig::from_json_str(custom_json).expect("Should parse custom JSON");
    let input_map = InputMap::new(&config);

    // Test Player 1 mapping
    let w_actions = input_map
        .get_actions(&Keycode::W)
        .expect("W should be mapped");
    assert!(w_actions.contains(&InputAction::Player1(JoypadButton::Up)));

    let j_actions = input_map
        .get_actions(&Keycode::J)
        .expect("J should be mapped");
    assert!(j_actions.contains(&InputAction::Player1(JoypadButton::A)));

    let k_actions = input_map
        .get_actions(&Keycode::K)
        .expect("K should be mapped");
    assert!(k_actions.contains(&InputAction::Player1(JoypadButton::B)));

    // Test Player 2 mapping
    let up_actions = input_map
        .get_actions(&Keycode::Up)
        .expect("ArrowUp should be mapped for P2");
    assert!(up_actions.contains(&InputAction::Player2(JoypadButton::Up)));

    // Test Hotkeys
    let space_actions = input_map
        .get_actions(&Keycode::Space)
        .expect("Space should be mapped to Pause");
    assert!(space_actions.contains(&InputAction::Hotkey(HotkeyAction::Pause)));

    let q_actions = input_map
        .get_actions(&Keycode::Q)
        .expect("Q should be mapped to Exit");
    assert!(q_actions.contains(&InputAction::Hotkey(HotkeyAction::Exit)));
}

#[test]
fn test_round_trip_serialization() {
    let original = ControlsConfig::default();
    let serialized = serde_json::to_string_pretty(&original).expect("Should serialize to JSON");
    let deserialized =
        ControlsConfig::from_json_str(&serialized).expect("Should deserialize from JSON");

    assert_eq!(
        original.player1.up.unwrap().keys(),
        deserialized.player1.up.unwrap().keys()
    );
    assert_eq!(
        original.player1.start.unwrap().keys(),
        deserialized.player1.start.unwrap().keys()
    );
    assert_eq!(
        original.player2.a.unwrap().keys(),
        deserialized.player2.a.unwrap().keys()
    );
    assert_eq!(
        original.hotkeys.pause.unwrap().keys(),
        deserialized.hotkeys.pause.unwrap().keys()
    );
}

#[test]
fn test_save_and_load_file() {
    let temp_dir = std::env::temp_dir();
    let file_path: PathBuf = temp_dir.join("test_controls_config.json");

    let original = ControlsConfig::default();
    original
        .save_to_file(&file_path)
        .expect("Should save to file");
    assert!(file_path.exists());

    let loaded = ControlsConfig::from_file(&file_path).expect("Should load from file");
    assert_eq!(
        original.player1.a.as_ref().unwrap().keys(),
        loaded.player1.a.as_ref().unwrap().keys()
    );

    let (cfg_load, path_used) = ControlsConfig::load_or_default(Some(&file_path));
    assert!(path_used.is_some());
    assert_eq!(
        cfg_load.player1.a.as_ref().unwrap().keys(),
        original.player1.a.as_ref().unwrap().keys()
    );

    // Clean up
    let _ = fs::remove_file(file_path);
}

#[test]
fn test_load_or_default_fallback_on_invalid() {
    // Non-existent path falls back to default without crashing
    let (config, path) = ControlsConfig::load_or_default(Some("non_existent_file_xyz_123.json"));
    assert!(path.is_none());
    assert!(config.player1.up.is_some());
}

#[test]
fn test_root_controls_json_parses_validly() {
    let root_controls = PathBuf::from("controls.json");
    if root_controls.exists() {
        let config =
            ControlsConfig::from_file(&root_controls).expect("controls.json must be valid");
        let input_map = InputMap::new(&config);

        // Verify that configured hotkeys are mapped
        assert!(input_map
            .get_actions(&Keycode::Escape)
            .unwrap()
            .contains(&InputAction::Hotkey(HotkeyAction::Exit)));

        // Verify that Player 1 has mapped controls
        assert!(config.player1.a.is_some());
        assert!(config.player1.b.is_some());

        // When player2 is omitted from controls.json, player2 should be empty
        assert!(config.player2.is_empty());

        // Key 'O' should only map to Player 1 B, not Player 2 A
        let o_actions = input_map
            .get_actions(&Keycode::O)
            .expect("O should be mapped");
        assert_eq!(o_actions, &[InputAction::Player1(JoypadButton::B)]);
    }
}

#[test]
fn test_one_player_config_omitted_player2_defaults_empty() {
    let json = r#"{
        "player1": {
            "up": ["Up"],
            "down": ["Down"],
            "left": ["Left"],
            "right": ["Right"],
            "a": ["A"],
            "b": ["O"],
            "select": ["E"],
            "start": "U"
        },
        "hotkeys": {
            "pause": "P",
            "exit": "Escape"
        }
    }"#;
    let config = ControlsConfig::from_json_str(json).expect("Should parse 1-player JSON");
    assert!(config.player2.is_empty());

    let input_map = InputMap::new(&config);

    // Key 'O' should ONLY trigger Player 1 B, NOT Player 2 A
    let o_actions = input_map
        .get_actions(&Keycode::O)
        .expect("O should be mapped");
    assert_eq!(o_actions, &[InputAction::Player1(JoypadButton::B)]);

    // Key 'U' should ONLY trigger Player 1 Start, NOT Player 2 B
    let u_actions = input_map
        .get_actions(&Keycode::U)
        .expect("U should be mapped");
    assert_eq!(u_actions, &[InputAction::Player1(JoypadButton::Start)]);
}

#[test]
fn test_player2_conflict_with_player1_is_resolved() {
    let json = r#"{
        "player1": {
            "a": "A",
            "b": "O"
        },
        "player2": {
            "a": "O",
            "b": "U"
        }
    }"#;
    let config = ControlsConfig::from_json_str(json).expect("Should parse JSON");
    let input_map = InputMap::new(&config);

    // Player 1 claims 'O'. Player 2's attempt to bind 'O' should be rejected.
    let o_actions = input_map
        .get_actions(&Keycode::O)
        .expect("O should be mapped");
    assert_eq!(o_actions, &[InputAction::Player1(JoypadButton::B)]);

    // Player 2 binding for 'U' should succeed since Player 1 does not use 'U'.
    let u_actions = input_map
        .get_actions(&Keycode::U)
        .expect("U should be mapped");
    assert_eq!(u_actions, &[InputAction::Player2(JoypadButton::B)]);
}

#[test]
fn test_contra_one_player_no_ghost_jump() {
    use nes::{Cartridge, Nes};
    let contra_path = "/Users/arunfelix/Documents/Games/NES/Contra (USA).nes";
    if !std::path::Path::new(contra_path).exists() {
        return;
    }

    let root_controls = PathBuf::from("controls.json");
    let config = if root_controls.exists() {
        ControlsConfig::from_file(&root_controls).unwrap()
    } else {
        ControlsConfig::default()
    };
    let input_map = InputMap::new(&config);

    let cart = Cartridge::from_file(contra_path).unwrap();
    let mut nes = Nes::new(cart);

    // Run past intro into title screen
    for _ in 0..100 {
        nes.step_frame();
    }
    // Press Start to begin 1-player game
    for _ in 0..10 {
        nes.set_button_p1(JoypadButton::Start, true);
        nes.step_frame();
    }
    for _ in 0..150 {
        nes.set_button_p1(JoypadButton::Start, false);
        nes.step_frame();
    }

    // Simulate pressing 'O' (the user's shoot button in 1-player config)
    if let Some(actions) = input_map.get_actions(&Keycode::O) {
        for &action in actions {
            match action {
                InputAction::Player1(btn) => nes.set_button_p1(btn, true),
                InputAction::Player2(btn) => nes.set_button_p2(btn, true),
                _ => {}
            }
        }
    }

    // Step frames to let game process controller input
    for _ in 0..5 {
        nes.step_frame();
    }

    // Contra RAM $F1 holds active player 1 button bits:
    // Bit 6 (0x40) = B button (Shoot)
    // Bit 7 (0x80) = A button (Jump)
    // Verify that RAM $F1 only has 0x40 (Shoot) and NOT 0x80 (Jump) or 0xC0 (Both)
    let f1 = nes.bus.cpu_ram[0xF1];
    assert_eq!(
        f1 & 0x80,
        0,
        "Player 1 should NOT jump when pressing 'O'! RAM $F1 was 0x{:02X}",
        f1
    );
    assert_eq!(
        f1 & 0x40,
        0x40,
        "Player 1 should shoot when pressing 'O'! RAM $F1 was 0x{:02X}",
        f1
    );
}
