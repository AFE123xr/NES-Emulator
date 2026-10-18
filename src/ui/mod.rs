pub mod screenshot;

use crate::audio::AudioPlayer;
use crate::controller::{ControlsConfig, HotkeyAction, InputAction, InputMap, JoypadButton};
use crate::nes::Nes;
use crate::netplay::{
    LockstepPoll, NetplayConfig, NetplayRole, NetplaySession, SessionState, INPUT_FLAG_PAUSE,
    INPUT_FLAG_RESET,
};
use crate::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
pub use screenshot::save_screenshot;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::video::FullscreenType;
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};

pub struct EmulatorConfig {
    pub scale: u32,
    pub title: String,
    pub debug_audio: bool,
    pub controls_path: Option<String>,
    /// Automatically save the session's audio as a WAV dump on exit.
    pub save_audio_on_exit: bool,
    /// Optional P2P online multiplayer configuration.
    pub netplay: Option<NetplayConfig>,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            scale: 3,
            title: "NES Emulator".to_string(),
            debug_audio: false,
            controls_path: None,
            save_audio_on_exit: false,
            netplay: None,
        }
    }
}

fn update_button_byte(buttons: &mut u8, btn: JoypadButton, pressed: bool) {
    let bit = 1u8 << (btn as u8);
    if pressed {
        *buttons |= bit;
    } else {
        *buttons &= !bit;
    }
}

pub fn run_emulator(mut nes: Nes, config: EmulatorConfig) -> Result<(), String> {
    let mut netplay_session = if let Some(ref net_cfg) = config.netplay {
        match net_cfg.role {
            NetplayRole::Host => {
                let session = NetplaySession::new_host(
                    net_cfg.port,
                    net_cfg.host_ip_override,
                    nes.rom_hash(),
                    net_cfg.input_delay,
                    net_cfg.ignore_rom_hash,
                )?;
                let primary_hash = session.invite_hash.encode();
                let local_hash = session.localhost_hash.encode();
                println!("============================================================");
                println!("🌐 P2P MULTIPLAYER HOST STARTED (Player 1)");
                println!("------------------------------------------------------------");
                println!("  Listening on:      0.0.0.0:{}", session.local_port());
                println!("  Advertised Peer:   {}", session.invite_hash.addr);
                println!("  ROM Checksum:      0x{:08X}", session.rom_hash);
                println!("  Input Delay:       {} frames", session.input_delay);
                println!("  Shareable Hash:    {}", primary_hash);
                if session.invite_hash.addr.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
                    println!("  Localhost Hash:    {} (same machine)", local_hash);
                }
                println!("------------------------------------------------------------");
                println!("Send the Shareable Hash to Player 2 to join with:");
                println!("  nes <rom_path> --join {}", primary_hash);
                println!("Waiting for Player 2 to connect...");
                println!("============================================================");
                Some(session)
            }
            NetplayRole::Peer => {
                let hash_str = net_cfg
                    .join_hash
                    .as_deref()
                    .ok_or_else(|| "Missing invite hash for --join".to_string())?;
                let session =
                    NetplaySession::new_peer(hash_str, nes.rom_hash(), net_cfg.ignore_rom_hash)?;
                println!("============================================================");
                println!("🌐 P2P MULTIPLAYER JOINING AS PLAYER 2");
                println!("------------------------------------------------------------");
                println!("  Target Host:       {}", session.invite_hash.addr);
                println!("  ROM Checksum:      0x{:08X}", session.rom_hash);
                println!("  Input Delay:       {} frames", session.input_delay);
                println!("  Invite Hash:       {}", session.invite_hash.encode());
                println!("------------------------------------------------------------");
                println!("Connecting to Host...");
                println!("============================================================");
                Some(session)
            }
        }
    } else {
        None
    };

    let sdl_context = sdl2::init().map_err(|e| format!("Failed to init SDL2: {}", e))?;
    let video_subsystem = sdl_context
        .video()
        .map_err(|e| format!("Failed to init SDL2 video: {}", e))?;

    let window_width = (SCREEN_WIDTH as u32) * config.scale;
    let window_height = (SCREEN_HEIGHT as u32) * config.scale;

    let window = video_subsystem
        .window(&config.title, window_width, window_height)
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| format!("Failed to create window: {}", e))?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| format!("Failed to create canvas: {}", e))?;

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            PixelFormatEnum::RGB24,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
        )
        .map_err(|e| format!("Failed to create texture: {}", e))?;

    // Audio setup with diagnostics
    let audio_player = if let Ok(audio_subsystem) = sdl_context.audio() {
        let sample_buffer = nes.bus.apu.sample_buffer.clone();
        Some(AudioPlayer::new(
            &audio_subsystem,
            sample_buffer,
            config.debug_audio,
        ))
    } else {
        None
    };

    // Load controls configuration
    let (controls_config, loaded_from) =
        ControlsConfig::load_or_default(config.controls_path.as_deref());
    if let Some(src) = &loaded_from {
        println!("🎮 Controls configured from: {}", src);
    }
    let input_map = InputMap::new(&controls_config);

    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|e| format!("Failed to create event pump: {}", e))?;

    let target_frame_duration = Duration::from_nanos(16_639_267); // ~60.0988 FPS
    let mut paused = false;
    let mut fast_forward = false;
    let mut is_fullscreen = false;
    let mut screenshot_notice: Option<(Instant, String)> = None;
    let mut audio_notice: Option<(Instant, String)> = None;
    let mut local_netplay_buttons: u8 = 0;
    let mut last_title_update = Instant::now() - Duration::from_secs(1);

    println!("NES Emulator started!");
    println!("Controls:");
    println!("  Player 1: D-Pad (Arrows/WASD), A (Z/K), B (X/J), Select (Space), Start (Enter)");
    println!("  Player 2: D-Pad (I/K/J/L), A (O), B (U), Select (Y), Start (H)");
    println!("Hotkeys:");
    println!("  P:      Pause / Resume");
    println!("  R:      Reset console");
    println!("  Tab:    Fast Forward (hold)");
    println!("  M:      Mute / Unmute Audio");
    println!("  F3:     Toggle Audio Diagnostics");
    println!("  F4:     Save Audio Dump (.wav saved to ./audio_dumps/)");
    println!("  F12/F2: Take Screenshot (saved to ./screenshots/)");
    println!("  F11:    Fullscreen Toggle");
    println!("  Esc:    Exit");

    'running: loop {
        let frame_start = Instant::now();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => {
                    let mut handled = false;
                    if let Some(actions) = input_map.get_actions(&key) {
                        for &action in actions {
                            handled = true;
                            match action {
                                InputAction::Player1(btn) => {
                                    if netplay_session.is_some() {
                                        update_button_byte(&mut local_netplay_buttons, btn, true);
                                    } else {
                                        nes.set_button_p1(btn, true);
                                    }
                                }
                                InputAction::Player2(btn) => {
                                    if netplay_session.is_some() {
                                        update_button_byte(&mut local_netplay_buttons, btn, true);
                                    } else {
                                        nes.set_button_p2(btn, true);
                                    }
                                }
                                InputAction::Hotkey(hotkey) => match hotkey {
                                    HotkeyAction::Exit => break 'running,
                                    HotkeyAction::Pause => {
                                        if let Some(ref mut session) = netplay_session {
                                            if session.state == SessionState::Connected {
                                                session.queue_local_flag(INPUT_FLAG_PAUSE);
                                            }
                                        } else {
                                            paused = !paused;
                                        }
                                    }
                                    HotkeyAction::Reset => {
                                        if let Some(ref mut session) = netplay_session {
                                            if session.state == SessionState::Connected {
                                                session.queue_local_flag(INPUT_FLAG_RESET);
                                            }
                                        } else {
                                            nes.reset();
                                            println!("Console reset.");
                                        }
                                    }
                                    HotkeyAction::FastForward => {
                                        if netplay_session.is_none() {
                                            fast_forward = true;
                                        }
                                    }
                                    HotkeyAction::Mute => {
                                        if let Some(player) = &audio_player {
                                            player.toggle_mute();
                                        }
                                    }
                                    HotkeyAction::AudioDiagnostics => {
                                        if let Some(player) = &audio_player {
                                            player.toggle_diagnostics();
                                        } else {
                                            println!("Audio subsystem not active.");
                                        }
                                    }
                                    HotkeyAction::AudioDump => {
                                        if let Some(player) = &audio_player {
                                            match player.save_wav(&config.title) {
                                                Ok(path) => {
                                                    println!("🔊 Audio buffer saved: {}", path);
                                                    audio_notice = Some((Instant::now(), path));
                                                }
                                                Err(e) => eprintln!("Audio save error: {}", e),
                                            }
                                        } else {
                                            println!("Audio subsystem not active.");
                                        }
                                    }
                                    HotkeyAction::Screenshot => {
                                        let frame = nes.bus.ppu.frame_buffer.as_ref();
                                        match screenshot::save_screenshot(frame, &config.title) {
                                            Ok(path) => {
                                                println!("📸 Screenshot captured: {}", path);
                                                screenshot_notice = Some((Instant::now(), path));
                                            }
                                            Err(e) => eprintln!("Screenshot error: {}", e),
                                        }
                                    }
                                    HotkeyAction::Fullscreen => {
                                        is_fullscreen = !is_fullscreen;
                                        let fs = if is_fullscreen {
                                            FullscreenType::Desktop
                                        } else {
                                            FullscreenType::Off
                                        };
                                        let _ = canvas.window_mut().set_fullscreen(fs);
                                    }
                                },
                            }
                        }
                    }
                    if !handled && key == Keycode::Escape {
                        break 'running;
                    }
                }
                Event::KeyUp {
                    keycode: Some(key), ..
                } => {
                    if let Some(actions) = input_map.get_actions(&key) {
                        for &action in actions {
                            match action {
                                InputAction::Player1(btn) => {
                                    if netplay_session.is_some() {
                                        update_button_byte(&mut local_netplay_buttons, btn, false);
                                    } else {
                                        nes.set_button_p1(btn, false);
                                    }
                                }
                                InputAction::Player2(btn) => {
                                    if netplay_session.is_some() {
                                        update_button_byte(&mut local_netplay_buttons, btn, false);
                                    } else {
                                        nes.set_button_p2(btn, false);
                                    }
                                }
                                InputAction::Hotkey(HotkeyAction::FastForward) => {
                                    fast_forward = false
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Update window title if screenshot or audio dump saved, or with live P2P status
        if let Some((time, _)) = &screenshot_notice {
            if time.elapsed().as_secs_f32() < 1.5 {
                let _ = canvas
                    .window_mut()
                    .set_title(&format!("[📸 Screenshot Saved!] {}", config.title));
            } else {
                let _ = canvas.window_mut().set_title(&config.title);
                screenshot_notice = None;
            }
        } else if let Some((time, _)) = &audio_notice {
            if time.elapsed().as_secs_f32() < 1.5 {
                let _ = canvas
                    .window_mut()
                    .set_title(&format!("[🔊 Audio Dump Saved!] {}", config.title));
            } else {
                let _ = canvas.window_mut().set_title(&config.title);
                audio_notice = None;
            }
        } else if let Some(ref session) = netplay_session {
            if last_title_update.elapsed() >= Duration::from_millis(250) {
                let _ = canvas.window_mut().set_title(&format!(
                    "[{}] {}",
                    session.status_badge(),
                    config.title
                ));
                last_title_update = Instant::now();
            }
        }

        if let Some(ref mut session) = netplay_session {
            let state_crc = nes.state_checksum();
            let mut poll_result = session.poll_step(local_netplay_buttons, 0, state_crc);

            // If connected and waiting on the remote peer's UDP packet for this frame,
            // briefly spin-wait up to 12ms so 60 FPS lockstep doesn't skip a beat.
            let wait_start = Instant::now();
            while matches!(poll_result, LockstepPoll::WaitingForRemoteInput { .. })
                && wait_start.elapsed() < Duration::from_millis(12)
            {
                std::thread::sleep(Duration::from_millis(1));
                poll_result = session.poll_step(local_netplay_buttons, 0, state_crc);
            }

            match poll_result {
                LockstepPoll::WaitingForHandshake => {
                    canvas.clear();
                    let _ = canvas.copy(&texture, None, None);
                    canvas.present();
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                LockstepPoll::WaitingForRemoteInput { .. } => {
                    continue;
                }
                LockstepPoll::Disconnected(reason) => {
                    println!("🌐 Netplay session ended: {}", reason);
                    break 'running;
                }
                LockstepPoll::FrameReady(sync_frame) => {
                    if sync_frame.reset_triggered {
                        nes.reset();
                        println!(
                            "🔄 Synchronized console reset at frame {}.",
                            sync_frame.frame
                        );
                    }
                    if sync_frame.pause_toggled {
                        paused = !paused;
                        println!(
                            "⏸️ Synchronized pause {} at frame {}.",
                            if paused { "enabled" } else { "resumed" },
                            sync_frame.frame
                        );
                    }
                    nes.set_buttons_p1(sync_frame.p1_buttons);
                    nes.set_buttons_p2(sync_frame.p2_buttons);
                }
            }
        }

        if paused {
            std::thread::sleep(Duration::from_millis(16));
            continue;
        }

        let frame = nes.step_frame();
        texture
            .update(None, frame.as_ref(), SCREEN_WIDTH * 3)
            .map_err(|e| format!("Failed to update texture: {}", e))?;

        canvas.clear();
        canvas
            .copy(&texture, None, None)
            .map_err(|e| format!("Failed to copy texture: {}", e))?;
        canvas.present();

        if !fast_forward {
            if audio_player.is_some() {
                // Audio-driven frame synchronization:
                // Tightly synchronize emulation pacing to the physical sound card
                // to eliminate buffer starvation (underruns) and overflow.
                let buf_len = nes
                    .bus
                    .apu
                    .sample_buffer
                    .lock()
                    .map(|b| b.len())
                    .unwrap_or(2500);

                if buf_len < 1800 {
                    // Buffer is running low (< 40ms): do not sleep.
                    // Immediately step the next frame to feed the audio buffer.
                } else if buf_len > 3500 {
                    // Buffer is running high (> 80ms): sleep slightly longer (18ms)
                    // so the audio hardware drains excess samples.
                    std::thread::sleep(Duration::from_millis(18));
                } else {
                    // Buffer is healthy (1800..=3500 samples / 40..=80ms):
                    // Pace to standard 60.0988 FPS.
                    let elapsed = frame_start.elapsed();
                    if elapsed < target_frame_duration {
                        std::thread::sleep(target_frame_duration - elapsed);
                    }
                }
            } else {
                let elapsed = frame_start.elapsed();
                if elapsed < target_frame_duration {
                    std::thread::sleep(target_frame_duration - elapsed);
                }
            }
        }
    }

    if let Some(ref mut session) = netplay_session {
        session.disconnect();
    }

    if let Some(player) = &audio_player {
        if config.save_audio_on_exit && player.diagnostics.sample_count() >= 4410 {
            match player.save_wav(&config.title) {
                Ok(path) => println!("🔊 Audio session saved to: {}", path),
                Err(e) => eprintln!("Failed to save audio on exit: {}", e),
            }
        }
        if player.diagnostics.is_enabled() {
            player.diagnostics.print_summary();
        }
    }

    Ok(())
}
