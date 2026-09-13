use nes::{Cartridge, EmulatorConfig, Nes};
use std::env;
use std::path::Path;
use std::process;
use std::time::Instant;

fn print_usage() {
    println!("NES Emulator (Rust)");
    println!();
    println!("Usage:");
    println!("  nes [rom_path] [options]");
    println!();
    println!("Environment Variables:");
    println!("  ROM_PATH, NES_ROM    Path to .nes ROM file (used if rom_path argument is omitted)");
    println!();
    println!("Options:");
    println!("  --scale <factor>     Window scale factor (1, 2, 3, 4, default: 3)");
    println!("  --headless <frames>  Run in headless mode for N frames (benchmarking/testing)");
    println!("  --debug-audio        Enable real-time audio diagnostics (diagnose popping & buffer health)");
    println!("  --help, -h           Show this help message");
    println!();
    println!("Controls:");
    println!("  D-Pad:               Arrow keys or WASD");
    println!("  A button:            Z or K");
    println!("  B button:            X or J");
    println!("  Select:              Space or Right Shift");
    println!("  Start:               Enter");
    println!();
    println!("Hotkeys:");
    println!("  F12, F2:             Capture Screenshot (saved to ./screenshots/)");
    println!("  F3:                  Toggle real-time Audio Diagnostics");
    println!("  F4:                  Save Audio Dump (.wav saved to ./audio_dumps/)");
    println!("  P:                   Pause / Resume");
    println!("  R:                   Reset console");
    println!("  Tab:                 Fast forward (hold)");
    println!("  M:                   Mute / Unmute audio");
    println!("  F11:                 Toggle fullscreen");
    println!("  Esc:                 Exit emulator (auto-saves audio dump)");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        return;
    }

    let mut rom_path_opt: Option<String> = None;
    let mut scale: u32 = 3;
    let mut headless_frames: Option<usize> = None;
    let mut debug_audio = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scale" => {
                if i + 1 < args.len() {
                    if let Ok(s) = args[i + 1].parse::<u32>() {
                        scale = s.clamp(1, 8);
                    }
                    i += 1;
                }
            }
            "--headless" => {
                if i + 1 < args.len() {
                    if let Ok(f) = args[i + 1].parse::<usize>() {
                        headless_frames = Some(f);
                    }
                    i += 1;
                }
            }
            "--debug-audio" => {
                debug_audio = true;
            }
            other => {
                if other.starts_with('-') {
                    eprintln!("Unknown option: {}", other);
                    print_usage();
                    process::exit(1);
                } else if rom_path_opt.is_none() {
                    rom_path_opt = Some(other.to_string());
                } else {
                    eprintln!("Unexpected argument: {}", other);
                    print_usage();
                    process::exit(1);
                }
            }
        }
        i += 1;
    }

    let rom_path = match rom_path_opt
        .or_else(|| env::var("ROM_PATH").ok())
        .or_else(|| env::var("NES_ROM").ok())
    {
        Some(path) => path,
        None => {
            eprintln!("Error: No ROM path specified.");
            eprintln!("Provide a ROM path as an argument or set the ROM_PATH / NES_ROM environment variable.\n");
            print_usage();
            process::exit(1);
        }
    };

    println!("Loading ROM: {}", rom_path);
    let cartridge = match Cartridge::from_file(&rom_path) {
        Ok(cart) => cart,
        Err(e) => {
            eprintln!("Error loading ROM '{}': {}", rom_path, e);
            process::exit(1);
        }
    };

    let rom_title = Path::new(&rom_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Game");

    println!(
        "Mapper: {}, PRG ROM: {} KB, CHR ROM: {} KB, Mirroring: {:?}",
        cartridge.header.mapper_id,
        cartridge.header.prg_rom_size / 1024,
        cartridge.header.chr_rom_size / 1024,
        cartridge.header.mirroring,
    );

    let mut nes = Nes::new(cartridge);

    if let Some(frames) = headless_frames {
        println!("Running headless for {} frames...", frames);
        let start = Instant::now();
        for _ in 0..frames {
            nes.step_frame();
        }
        let elapsed = start.elapsed();
        let fps = (frames as f64) / elapsed.as_secs_f64();
        println!(
            "Completed {} frames in {:.3}s ({:.1} FPS)",
            frames,
            elapsed.as_secs_f64(),
            fps
        );
        return;
    }

    let config = EmulatorConfig {
        scale,
        title: format!("NES Emulator - {}", rom_title),
        debug_audio,
    };

    if let Err(e) = nes::run_emulator(nes, config) {
        eprintln!("Emulator error: {}", e);
        process::exit(1);
    }
}
