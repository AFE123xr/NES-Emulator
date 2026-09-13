use nes::save_screenshot;
use nes::Cartridge;
use nes::JoypadButton;
use nes::Nes;

#[test]
fn test_zelda_navigate_to_game() {
    let rom_path = match std::env::var("ROM_PATH").or_else(|_| std::env::var("NES_ROM")) {
        Ok(path) => path,
        Err(_) => {
            eprintln!("ROM_PATH or NES_ROM environment variable not set, skipping test");
            return;
        }
    };
    if !std::path::Path::new(&rom_path).exists() {
        eprintln!("ROM not found at {}, skipping test", rom_path);
        return;
    }
    let cartridge = Cartridge::from_file(&rom_path).expect("Failed to load ROM");
    let mut nes = Nes::new(cartridge);

    // Title screen
    for _ in 0..150 {
        nes.step_frame();
    }

    // Start -> File Select
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);

    for _ in 0..60 {
        nes.step_frame();
    }

    // File select: Slot 1 is Link!
    // In Zelda, Slot 1 is already created with name "LINK" or empty?
    // Let's check what is on the file select screen:
    // Slot 1 has a little Link icon!
    // If you press Select on File Select screen, the cursor moves:
    // Slot 1 -> Slot 2 -> Slot 3 -> Register Your Name -> Elimination Mode!
    // If Slot 1 is empty, pressing Start on Register Your Name goes to Register Name.
    // Let's capture file select:
    let frame = nes.step_frame();
    save_screenshot(frame, "zelda_menu_start").unwrap();

    // First, enter Register Name:
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);
    for _ in 0..60 {
        nes.step_frame();
    }

    // Heart is on Slot 1. Press Start to start entering name for Slot 1!
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);
    for _ in 0..30 {
        nes.step_frame();
    }

    // Type 'A'
    nes.set_button_p1(JoypadButton::A, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::A, false);
    for _ in 0..30 {
        nes.step_frame();
    }

    // Press Select 1: moves to Link 2
    nes.set_button_p1(JoypadButton::Select, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Select, false);
    for _ in 0..30 {
        nes.step_frame();
    }
    save_screenshot(nes.step_frame(), "zelda_sel_1").unwrap();

    // Press Select 2: moves to Link 3
    nes.set_button_p1(JoypadButton::Select, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Select, false);
    for _ in 0..30 {
        nes.step_frame();
    }
    save_screenshot(nes.step_frame(), "zelda_sel_2").unwrap();

    // Press Select 3: moves to END?
    nes.set_button_p1(JoypadButton::Select, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Select, false);
    for _ in 0..30 {
        nes.step_frame();
    }
    save_screenshot(nes.step_frame(), "zelda_sel_3").unwrap();

    // Press Start on END!
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);
    for _ in 0..60 {
        nes.step_frame();
    }
    save_screenshot(nes.step_frame(), "zelda_sel_after_start").unwrap();

    let frame = nes.step_frame();
    save_screenshot(frame, "zelda_back_to_menu").unwrap();

    // Now Slot 1 has name "A"! The cursor is on Slot 1!
    // Press Start to enter the game!
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);

    // Step into overworld
    for frame_idx in 1..=120 {
        let frame = nes.step_frame();
        if frame_idx % 60 == 0 {
            let name = format!("zelda_game_{}", frame_idx);
            let path = save_screenshot(frame, &name).unwrap();
            println!("Saved overworld gameplay {} to {}", frame_idx, path);
        }
    }

    // Walk Link to the right (into next screen)
    println!("Walking Link right...");
    nes.set_button_p1(JoypadButton::Right, true);
    for frame_idx in 1..=300 {
        let frame = nes.step_frame();
        if frame_idx % 30 == 0 {
            let name = format!("zelda_walk_right_{}", frame_idx);
            let path = save_screenshot(frame, &name).unwrap();
            println!("Saved walk right {} to {}", frame_idx, path);
        }
    }
    nes.set_button_p1(JoypadButton::Right, false);

    // After scrolling into next room, step a few more frames
    for frame_idx in 1..=60 {
        let frame = nes.step_frame();
        if frame_idx == 60 {
            save_screenshot(frame, "zelda_room_right_final").unwrap();
        }
    }
}
