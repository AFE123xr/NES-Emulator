use nes::save_screenshot;
use nes::Cartridge;
use nes::JoypadButton;
use nes::Nes;
use std::fs;
use std::path::Path;
use std::process::Command;

fn capture_game(
    rom_name: &str,
    out_name: &str,
    frames: usize,
    press_start_at: Option<usize>,
    play_frames: usize,
) {
    let rom_dir = match std::env::var("NES_ROMS_DIR") {
        Ok(dir) => dir,
        Err(_) => {
            eprintln!("NES_ROMS_DIR environment variable not set, skipping screenshot capture");
            return;
        }
    };
    let rom_path = Path::new(&rom_dir).join(rom_name);
    if !rom_path.exists() {
        eprintln!(
            "ROM not found at {:?}, skipping screenshot capture",
            rom_path
        );
        return;
    }

    let cart = Cartridge::from_file(&rom_path).expect("load cart");
    let mut nes = Nes::new(cart);

    for f in 0..frames {
        nes.step_frame();
        if let Some(start_f) = press_start_at {
            if f == start_f {
                nes.set_button_p1(JoypadButton::Start, true);
            } else if f == start_f + 8 {
                nes.set_button_p1(JoypadButton::Start, false);
            }
        }
    }

    for _ in 0..play_frames {
        nes.step_frame();
    }

    let frame = nes.step_frame();
    let _ = fs::create_dir_all("screenshots");
    let _ = fs::create_dir_all("images");

    save_screenshot(frame, out_name).unwrap();

    // Find the latest generated bmp in screenshots/ and copy/convert to images/{out_name}.png
    // Or save directly
    let mut entries: Vec<_> = fs::read_dir("screenshots")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with(out_name)
                && e.file_name().to_string_lossy().ends_with(".bmp")
        })
        .collect();
    entries.sort_by_key(|e| e.metadata().unwrap().modified().unwrap());

    if let Some(latest) = entries.last() {
        let png_path = format!("images/{}.png", out_name);
        let status = Command::new("sips")
            .args([
                "-s",
                "format",
                "png",
                latest.path().to_str().unwrap(),
                "--out",
                &png_path,
            ])
            .output();
        if let Ok(s) = status {
            if s.status.success() {
                println!("Successfully generated {}", png_path);
            } else {
                eprintln!("sips error: {:?}", String::from_utf8_lossy(&s.stderr));
            }
        }
    }
}

#[test]
fn generate_all_readme_screenshots() {
    let _ = fs::create_dir_all("images");

    // 1. Super Mario Bros. (World 1-1 Gameplay)
    capture_game(
        "Super Mario Bros. (World).nes",
        "mario_gameplay",
        100,
        Some(80),
        60,
    );

    // 2. Super Mario Bros. (Title Screen)
    capture_game("Super Mario Bros. (World).nes", "mario_title", 90, None, 0);

    // 3. The Legend of Zelda (Title Screen)
    capture_game(
        "Legend of Zelda, The (USA).nes",
        "zelda_title",
        120,
        None,
        0,
    );

    // 4. Mega Man 2 (Title Screen)
    capture_game("Mega Man 2 (USA).nes", "megaman2_title", 150, None, 0);

    // 5. Castlevania (Title Screen)
    capture_game("Castlevania (USA).nes", "castlevania_title", 120, None, 0);

    // 6. Contra (Title Screen)
    capture_game("Contra (USA).nes", "contra_title", 120, None, 0);

    // 7. Super Mario Bros. 3 (Title Screen)
    capture_game(
        "Super Mario Bros. 3 (USA) (Rev 1).nes",
        "smb3_title",
        180,
        None,
        0,
    );

    // 8. Zelda II (Title Screen)
    capture_game(
        "Zelda II - The Adventure of Link (USA).nes",
        "zelda2_title",
        120,
        None,
        0,
    );
}
