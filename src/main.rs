use nes::{
    Cartridge, CheatsConfig, ControlsConfig, EmulatorConfig, LockstepPoll, Nes, NetplayConfig,
    NetplayRole, NetplaySession, DEFAULT_INPUT_DELAY, DEFAULT_NETPLAY_PORT,
};
use std::env;
use std::net::IpAddr;
use std::path::Path;
use std::process;
use std::time::{Duration, Instant};

fn print_usage() {
    println!("NES Emulator (Rust)");
    println!();
    println!("Usage:");
    println!("  nes [rom_path] [options]");
    println!();
    println!("Environment Variables:");
    println!(
        "  ROM_PATH, NES_ROM        Path to .nes ROM file (used if rom_path argument is omitted)"
    );
    println!("  NES_CONTROLS             Path to custom controls JSON file");
    println!("  NES_CHEATS               Path to custom cheats JSON file");
    println!();
    println!("Options:");
    println!("  --scale <factor>         Window scale factor (1, 2, 3, 4, default: 3)");
    println!("  --headless <frames>      Run in headless mode for N frames (benchmarking/testing)");
    println!("  --debug-audio            Enable real-time audio diagnostics (diagnose popping & buffer health)");
    println!(
        "  --save-audio-on-exit     Save the session audio as a WAV dump (./audio_dumps/) on exit"
    );
    println!("  --controls <file>        Path to custom controls JSON file (default: ./controls.json if present)");
    println!("  --config <file>          Alias for --controls");
    println!(
        "  --generate-controls [f]  Generate default controls JSON file (default: controls.json)"
    );
    println!("  --cheats <file>          Path to custom cheats JSON file (default: ./cheats.json if present)");
    println!("  --no-cheats              Disable all cheats for this session");
    println!("  --generate-cheats [f]    Generate default cheats JSON file (default: cheats.json)");
    println!("  --help, -h               Show this help message");
    println!();
    println!("P2P Online Multiplayer Options:");
    println!(
        "  --host [port]            Host a P2P session as Player 1 (default UDP port: {})",
        DEFAULT_NETPLAY_PORT
    );
    println!(
        "                           Prints a custom shareable hash (NES-XXXXXX-...) for Player 2"
    );
    println!(
        "  --host-ip <ip>           Override the IP address encoded into the shareable host hash"
    );
    println!(
        "  --join <hash>            Join a P2P session as Player 2 using the host's custom hash"
    );
    println!(
        "  --delay <frames>         Lockstep input delay in frames when hosting (1-10, default: {})",
        DEFAULT_INPUT_DELAY
    );
    println!("  --ignore-rom-hash        Allow P2P connection even if ROM checksums differ");
    println!();
    println!("Default Controls (configurable via controls.json):");
    println!("  Player 1 (and Local Player in P2P Netplay):");
    println!("    D-Pad:                 Arrow keys or WASD");
    println!("    A button:              Z or K");
    println!("    B button:              X or J");
    println!("    Select:                Space or Right Shift");
    println!("    Start:                 Enter");
    println!("  Player 2 (Local 2-Player):");
    println!("    D-Pad:                 I, K, J, L");
    println!("    A button:              O");
    println!("    B button:              U");
    println!("    Select:                Y");
    println!("    Start:                 H");
    println!();
    println!("Hotkeys:");
    println!("  F12, F2:                 Capture Screenshot (saved to ./screenshots/)");
    println!("  F3:                      Toggle real-time Audio Diagnostics");
    println!("  F4:                      Save Audio Dump (.wav saved to ./audio_dumps/)");
    println!("  F8:                      Toggle Cheats On / Off");
    println!("  P:                       Pause / Resume");
    println!("  R:                       Reset console");
    println!("  Tab:                     Fast forward (hold)");
    println!("  M:                       Mute / Unmute audio");
    println!("  F11:                     Toggle fullscreen");
    println!("  Esc:                     Exit emulator");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        return;
    }

    let mut rom_path_opt: Option<String> = None;
    let mut controls_path_opt: Option<String> = None;
    let mut cheats_path_opt: Option<String> = None;
    let mut no_cheats = false;
    let mut scale: u32 = 3;
    let mut headless_frames: Option<usize> = None;
    let mut debug_audio = false;
    let mut save_audio_on_exit = false;

    let mut netplay_role: Option<NetplayRole> = None;
    let mut netplay_port: u16 = DEFAULT_NETPLAY_PORT;
    let mut netplay_host_ip: Option<IpAddr> = None;
    let mut netplay_join_hash: Option<String> = None;
    let mut netplay_delay: u8 = DEFAULT_INPUT_DELAY;
    let mut ignore_rom_hash = false;

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
            "--save-audio-on-exit" => {
                save_audio_on_exit = true;
            }
            "--host" => {
                netplay_role = Some(NetplayRole::Host);
                if i + 1 < args.len() {
                    if let Ok(port) = args[i + 1].parse::<u16>() {
                        netplay_port = port;
                        i += 1;
                    }
                }
            }
            "--host-ip" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<IpAddr>() {
                        Ok(ip) => {
                            netplay_host_ip = Some(ip);
                            i += 1;
                        }
                        Err(e) => {
                            eprintln!("Error: Invalid IP address '{}': {}", args[i + 1], e);
                            process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Error: Missing IP address after --host-ip");
                    process::exit(1);
                }
            }
            "--join" | "--connect" => {
                if i + 1 < args.len() {
                    netplay_role = Some(NetplayRole::Peer);
                    netplay_join_hash = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: Missing invite hash after {}", args[i]);
                    print_usage();
                    process::exit(1);
                }
            }
            "--delay" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<u8>() {
                        Ok(d) => {
                            netplay_delay = d.clamp(1, 10);
                            i += 1;
                        }
                        Err(_) => {
                            eprintln!("Error: Invalid frame delay '{}'", args[i + 1]);
                            process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Error: Missing frame count after --delay");
                    process::exit(1);
                }
            }
            "--ignore-rom-hash" => {
                ignore_rom_hash = true;
            }
            "--controls" | "--config" => {
                if i + 1 < args.len() {
                    controls_path_opt = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: Missing file path after {}", args[i]);
                    print_usage();
                    process::exit(1);
                }
            }
            "--generate-controls" => {
                let target = if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    args[i].clone()
                } else {
                    "controls.json".to_string()
                };
                let default_config = ControlsConfig::default();
                match default_config.save_to_file(&target) {
                    Ok(()) => {
                        println!("Created default controls configuration file at: {}", target);
                        return;
                    }
                    Err(e) => {
                        eprintln!("Error generating controls configuration: {}", e);
                        process::exit(1);
                    }
                }
            }
            "--cheats" => {
                if i + 1 < args.len() {
                    cheats_path_opt = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: Missing file path after {}", args[i]);
                    print_usage();
                    process::exit(1);
                }
            }
            "--no-cheats" => {
                no_cheats = true;
            }
            "--generate-cheats" => {
                let target = if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    args[i].clone()
                } else {
                    "cheats.json".to_string()
                };
                let default_cheats = CheatsConfig::default();
                match default_cheats.save_to_file(&target) {
                    Ok(()) => {
                        println!("Created default cheats configuration file at: {}", target);
                        return;
                    }
                    Err(e) => {
                        eprintln!("Error generating cheats configuration: {}", e);
                        process::exit(1);
                    }
                }
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

    let controls_path = controls_path_opt.or_else(|| env::var("NES_CONTROLS").ok());
    let cheats_path = cheats_path_opt.or_else(|| env::var("NES_CHEATS").ok());

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
        "Mapper: {}, PRG ROM: {} KB, CHR ROM: {} KB, Mirroring: {:?}, ROM Hash: 0x{:08X}",
        cartridge.header.mapper_id,
        cartridge.header.prg_rom_size / 1024,
        cartridge.header.chr_rom_size / 1024,
        cartridge.header.mirroring,
        cartridge.rom_hash,
    );

    let netplay = netplay_role.map(|role| NetplayConfig {
        role,
        port: netplay_port,
        host_ip_override: netplay_host_ip,
        join_hash: netplay_join_hash,
        input_delay: netplay_delay,
        ignore_rom_hash,
    });

    let mut nes = Nes::new(cartridge);

    if !no_cheats {
        let (cheats_cfg, cheats_src) = CheatsConfig::load_or_empty(cheats_path.as_deref());
        let (controls_cfg, controls_src) =
            ControlsConfig::load_or_default(controls_path.as_deref());
        let mut all_cheats = cheats_cfg.cheats;
        for c in &controls_cfg.cheats {
            if !all_cheats.contains(c) {
                all_cheats.push(c.clone());
            }
        }
        if !all_cheats.is_empty() {
            let active_count = all_cheats.iter().filter(|c| c.enabled).count();
            if let Some(src) = &cheats_src {
                println!(
                    "✨ Cheats loaded from {}: {} active cheat(s)",
                    src, active_count
                );
            } else if let Some(src) = &controls_src {
                println!(
                    "✨ Cheats loaded from {}: {} active cheat(s)",
                    src, active_count
                );
            }
            for cheat in all_cheats.iter().filter(|c| c.enabled) {
                let label = if cheat.name.is_empty() {
                    "Unnamed cheat"
                } else {
                    &cheat.name
                };
                println!(
                    "   • {} -> [0x{:04X}] = 0x{:02X}",
                    label, cheat.address, cheat.value
                );
            }
            nes.set_cheats(all_cheats);
        }
    }

    if let Some(frames) = headless_frames {
        if let Some(net_cfg) = netplay {
            let mut session = match net_cfg.role {
                NetplayRole::Host => match NetplaySession::new_host(
                    net_cfg.port,
                    net_cfg.host_ip_override,
                    nes.rom_hash(),
                    net_cfg.input_delay,
                    net_cfg.ignore_rom_hash,
                ) {
                    Ok(s) => {
                        println!("Shareable Hash: {}", s.invite_hash.encode());
                        println!("Localhost Hash: {}", s.localhost_hash.encode());
                        s
                    }
                    Err(e) => {
                        eprintln!("Netplay host error: {}", e);
                        process::exit(1);
                    }
                },
                NetplayRole::Peer => {
                    let hash_str = net_cfg.join_hash.as_deref().unwrap_or("");
                    match NetplaySession::new_peer(
                        hash_str,
                        nes.rom_hash(),
                        net_cfg.ignore_rom_hash,
                    ) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("Netplay join error: {}", e);
                            process::exit(1);
                        }
                    }
                }
            };

            println!("Running headless P2P netplay for {} frames...", frames);
            let start = Instant::now();
            let mut completed = 0;
            while completed < frames {
                match session.poll_step(0, 0, nes.state_checksum()) {
                    LockstepPoll::WaitingForHandshake
                    | LockstepPoll::WaitingForRemoteInput { .. } => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    LockstepPoll::FrameReady(sync) => {
                        if sync.reset_triggered {
                            nes.reset();
                        }
                        nes.set_buttons_p1(sync.p1_buttons);
                        nes.set_buttons_p2(sync.p2_buttons);
                        nes.step_frame();
                        completed += 1;
                    }
                    LockstepPoll::Disconnected(reason) => {
                        eprintln!("Netplay disconnected at frame {}: {}", completed, reason);
                        process::exit(1);
                    }
                }
            }
            session.disconnect();
            let elapsed = start.elapsed();
            println!(
                "Completed {} synchronized P2P frames in {:.3}s (Final Checksum: 0x{:08X})",
                frames,
                elapsed.as_secs_f64(),
                nes.state_checksum()
            );
            return;
        }

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
        controls_path,
        cheats_path,
        no_cheats,
        save_audio_on_exit,
        netplay,
    };

    if let Err(e) = nes::run_emulator(nes, config) {
        eprintln!("Emulator error: {}", e);
        process::exit(1);
    }
}
