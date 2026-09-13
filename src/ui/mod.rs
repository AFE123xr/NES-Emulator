pub mod screenshot;

use crate::audio::AudioPlayer;
use crate::controller::JoypadButton;
use crate::nes::Nes;
use crate::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
pub use screenshot::save_screenshot;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::video::FullscreenType;
use std::time::{Duration, Instant};

pub struct EmulatorConfig {
    pub scale: u32,
    pub title: String,
    pub debug_audio: bool,
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self {
            scale: 3,
            title: "NES Emulator".to_string(),
            debug_audio: false,
        }
    }
}

pub fn run_emulator(mut nes: Nes, config: EmulatorConfig) -> Result<(), String> {
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

    let mut event_pump = sdl_context
        .event_pump()
        .map_err(|e| format!("Failed to create event pump: {}", e))?;

    let target_frame_duration = Duration::from_nanos(16_639_267); // ~60.0988 FPS
    let mut paused = false;
    let mut fast_forward = false;
    let mut is_fullscreen = false;
    let mut screenshot_notice: Option<(Instant, String)> = None;
    let mut audio_notice: Option<(Instant, String)> = None;

    println!("NES Emulator started!");
    println!("Controls:");
    println!("  D-Pad:  Arrow Keys or WASD");
    println!("  A:      Z or K");
    println!("  B:      X or J");
    println!("  Select: Space or Right Shift");
    println!("  Start:  Enter");
    println!("Hotkeys:");
    println!("  P:      Pause / Resume");
    println!("  R:      Reset console");
    println!("  Tab:    Fast Forward (hold)");
    println!("  M:      Mute / Unmute Audio");
    println!("  F3:     Toggle Audio Diagnostics (diagnose popping & buffer health)");
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
                } => match key {
                    Keycode::Escape => break 'running,
                    Keycode::P => paused = !paused,
                    Keycode::R => {
                        nes.reset();
                        println!("Console reset.");
                    }
                    Keycode::Tab => fast_forward = true,
                    Keycode::M => {
                        if let Some(player) = &audio_player {
                            player.toggle_mute();
                        }
                    }
                    Keycode::F3 => {
                        if let Some(player) = &audio_player {
                            player.toggle_diagnostics();
                        } else {
                            println!("Audio subsystem not active.");
                        }
                    }
                    Keycode::F4 => {
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
                    Keycode::F12 | Keycode::F2 => {
                        let frame = nes.bus.ppu.frame_buffer.as_ref();
                        match screenshot::save_screenshot(frame, &config.title) {
                            Ok(path) => {
                                println!("📸 Screenshot captured: {}", path);
                                screenshot_notice = Some((Instant::now(), path));
                            }
                            Err(e) => eprintln!("Screenshot error: {}", e),
                        }
                    }
                    Keycode::F11 => {
                        is_fullscreen = !is_fullscreen;
                        let fs = if is_fullscreen {
                            FullscreenType::Desktop
                        } else {
                            FullscreenType::Off
                        };
                        let _ = canvas.window_mut().set_fullscreen(fs);
                    }
                    // Player 1 input
                    Keycode::Up | Keycode::W => nes.set_button_p1(JoypadButton::Up, true),
                    Keycode::Down | Keycode::S => nes.set_button_p1(JoypadButton::Down, true),
                    Keycode::Left | Keycode::A => nes.set_button_p1(JoypadButton::Left, true),
                    Keycode::Right | Keycode::D => nes.set_button_p1(JoypadButton::Right, true),
                    Keycode::Z | Keycode::K => nes.set_button_p1(JoypadButton::A, true),
                    Keycode::X | Keycode::J => nes.set_button_p1(JoypadButton::B, true),
                    Keycode::Space | Keycode::RShift => {
                        nes.set_button_p1(JoypadButton::Select, true)
                    }
                    Keycode::Return => nes.set_button_p1(JoypadButton::Start, true),
                    _ => {}
                },
                Event::KeyUp {
                    keycode: Some(key), ..
                } => match key {
                    Keycode::Tab => fast_forward = false,
                    Keycode::Up | Keycode::W => nes.set_button_p1(JoypadButton::Up, false),
                    Keycode::Down | Keycode::S => nes.set_button_p1(JoypadButton::Down, false),
                    Keycode::Left | Keycode::A => nes.set_button_p1(JoypadButton::Left, false),
                    Keycode::Right | Keycode::D => nes.set_button_p1(JoypadButton::Right, false),
                    Keycode::Z | Keycode::K => nes.set_button_p1(JoypadButton::A, false),
                    Keycode::X | Keycode::J => nes.set_button_p1(JoypadButton::B, false),
                    Keycode::Space | Keycode::RShift => {
                        nes.set_button_p1(JoypadButton::Select, false)
                    }
                    Keycode::Return => nes.set_button_p1(JoypadButton::Start, false),
                    _ => {}
                },
                _ => {}
            }
        }

        // Update window title if screenshot or audio dump saved
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
        }

        if paused {
            std::thread::sleep(Duration::from_millis(20));
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

    if let Some(player) = &audio_player {
        if player.diagnostics.sample_count() >= 4410 {
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
