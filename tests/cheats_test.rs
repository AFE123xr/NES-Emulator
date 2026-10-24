use nes::cpu::Memory;
use nes::{Cartridge, CheatEntry, CheatsConfig, ControlsConfig, JoypadButton, Nes};
use std::path::PathBuf;

fn create_dummy_rom() -> Cartridge {
    let mut rom = vec![0u8; 16 + 16384 + 8192];
    rom[0..4].copy_from_slice(b"NES\x1A");
    rom[4] = 1; // 16KB PRG ROM
    rom[5] = 1; // 8KB CHR ROM
    rom[6] = 0x00;

    // Simple program at $8000 that tries to overwrite $00AA with $00 every loop:
    // LDA #$00
    // STA $00AA
    // JMP $8000
    let prg_code: &[u8] = &[
        0xA9, 0x00, // LDA #$00
        0x85, 0xAA, // STA $AA
        0x4C, 0x00, 0x80, // JMP $8000
    ];
    rom[16..16 + prg_code.len()].copy_from_slice(prg_code);
    rom[16 + 0x3FFC] = 0x00;
    rom[16 + 0x3FFD] = 0x80;

    Cartridge::from_bytes(&rom).expect("valid test ROM")
}

#[test]
fn test_root_cheats_json_loads_contra_spread_rapid() {
    let path = PathBuf::from("cheats.json");
    assert!(path.exists(), "cheats.json should exist in project root");

    let cfg = CheatsConfig::from_file(&path).expect("cheats.json should parse cleanly");
    let active = cfg.active_cheats();
    assert!(!active.is_empty());

    // Verify Player 1 Contra Spread Gun + Rapid Fire (0x00AA = 0x13)
    let p1_cheat = active
        .iter()
        .find(|c| c.address == 0x00AA)
        .expect("Should have cheat for Contra P1 weapon address 0x00AA");
    assert_eq!(p1_cheat.value, 0x13);
}

#[test]
fn test_cheats_json_formats_and_roundtrip() {
    // Hex with 0x, $, plain hex, and decimal integers
    let json = r#"{
        "cheats": [
            {
                "name": "Contra P1 Spread + Rapid",
                "address": "0x00AA",
                "value": "0x13"
            },
            {
                "name": "Dollar Hex Format",
                "address": "$00AB",
                "value": "$13",
                "compare": "$00",
                "enabled": true
            },
            {
                "name": "Numeric Format",
                "address": 170,
                "value": 19,
                "enabled": false
            }
        ]
    }"#;

    let cfg = CheatsConfig::from_json_str(json).expect("Should parse mixed format cheats");
    assert_eq!(cfg.cheats.len(), 3);
    assert_eq!(cfg.cheats[0].address, 0x00AA);
    assert_eq!(cfg.cheats[0].value, 0x13);
    assert!(cfg.cheats[0].enabled);

    assert_eq!(cfg.cheats[1].address, 0x00AB);
    assert_eq!(cfg.cheats[1].value, 0x13);
    assert_eq!(cfg.cheats[1].compare, Some(0x00));

    assert_eq!(cfg.cheats[2].address, 170);
    assert_eq!(cfg.cheats[2].value, 19);
    assert!(!cfg.cheats[2].enabled);

    assert_eq!(cfg.active_cheats().len(), 2);

    // Also support top-level JSON array format
    let array_json = r#"[
        { "name": "Direct List Cheat", "address": "0x00AA", "value": "0x13" }
    ]"#;
    let list_cfg = CheatsConfig::from_json_str(array_json).expect("Should parse array format");
    assert_eq!(list_cfg.cheats.len(), 1);
    assert_eq!(list_cfg.cheats[0].address, 0x00AA);
    assert_eq!(list_cfg.cheats[0].value, 0x13);
}

#[test]
fn test_controls_config_can_embed_cheats() {
    let json = r#"{
        "player1": { "a": "Z", "b": "X" },
        "cheats": [
            {
                "name": "Contra Spread Rapid",
                "address": "0x00AA",
                "value": "0x13"
            }
        ]
    }"#;
    let cfg = ControlsConfig::from_json_str(json).expect("Should parse controls with cheats");
    assert_eq!(cfg.cheats.len(), 1);
    assert_eq!(cfg.cheats[0].address, 0x00AA);
    assert_eq!(cfg.cheats[0].value, 0x13);
}

#[test]
fn test_memory_cheat_freezes_ram_and_toggles() {
    let cart = create_dummy_rom();
    let mut nes = Nes::new(cart);

    nes.add_cheat(CheatEntry::new("Contra P1 Spread + Rapid", 0x00AA, 0x13));

    // Even though the CPU loop continually writes 0x00 to $00AA, the cheat keeps it at 0x13
    nes.step_frame();
    assert_eq!(nes.bus.cpu_ram[0x00AA], 0x13);
    assert_eq!(nes.bus.mem_read(0x00AA), 0x13);

    // Toggling cheats off allows the CPU to write 0x00 to $00AA
    let enabled = nes.toggle_cheats();
    assert!(!enabled);
    nes.step_frame();
    assert_eq!(nes.bus.cpu_ram[0x00AA], 0x00);

    // Toggling cheats back on immediately restores 0x13
    let enabled = nes.toggle_cheats();
    assert!(enabled);
    assert_eq!(nes.bus.cpu_ram[0x00AA], 0x13);
}

#[test]
fn test_contra_spread_gun_with_rapid_fire_in_game() {
    let contra_path = "/Users/arunfelix/Documents/Games/NES/Contra (USA).nes";
    if !std::path::Path::new(contra_path).exists() {
        return;
    }

    let cheats_cfg = CheatsConfig::from_file("cheats.json").expect("cheats.json must be valid");
    let cart = Cartridge::from_file(contra_path).unwrap();
    let mut nes = Nes::new(cart);
    nes.set_cheats(cheats_cfg.cheats);

    // Step past intro into title screen
    for _ in 0..100 {
        nes.step_frame();
    }
    // Press Start to begin 1-player game
    for _ in 0..10 {
        nes.set_button_p1(JoypadButton::Start, true);
        nes.step_frame();
    }
    for _ in 0..200 {
        nes.set_button_p1(JoypadButton::Start, false);
        nes.step_frame();
    }

    // Verify Player 1 & Player 2 have Spread Gun (low nibble 0x03) + Rapid Fire (bit 4 = 0x10) -> 0x13
    assert_eq!(
        nes.bus.cpu_ram[0x00AA], 0x13,
        "Contra Player 1 weapon at $00AA should be 0x13 (Spread Gun + Rapid Fire)"
    );
    assert_eq!(
        nes.bus.cpu_ram[0x00AB], 0x13,
        "Contra Player 2 weapon at $00AB should be 0x13 (Spread Gun + Rapid Fire)"
    );
}
