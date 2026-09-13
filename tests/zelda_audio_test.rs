use nes::Cartridge;
use nes::JoypadButton;
use nes::Nes;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn write_wav(samples: &[f32], path: &str) {
    let dir = Path::new("audio_dumps");
    if !dir.exists() {
        let _ = fs::create_dir_all(dir);
    }
    let num_samples = samples.len() as u32;
    let subchunk2_size = num_samples * 2;
    let chunk_size = 36 + subchunk2_size;
    let sample_rate = 44100u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;
    let bits_per_sample = 16u16;

    let mut wav_data = Vec::with_capacity(44 + subchunk2_size as usize);
    wav_data.extend_from_slice(b"RIFF");
    wav_data.extend_from_slice(&chunk_size.to_le_bytes());
    wav_data.extend_from_slice(b"WAVE");

    wav_data.extend_from_slice(b"fmt ");
    wav_data.extend_from_slice(&16u32.to_le_bytes());
    wav_data.extend_from_slice(&1u16.to_le_bytes());
    wav_data.extend_from_slice(&1u16.to_le_bytes());
    wav_data.extend_from_slice(&sample_rate.to_le_bytes());
    wav_data.extend_from_slice(&byte_rate.to_le_bytes());
    wav_data.extend_from_slice(&block_align.to_le_bytes());
    wav_data.extend_from_slice(&bits_per_sample.to_le_bytes());

    wav_data.extend_from_slice(b"data");
    wav_data.extend_from_slice(&subchunk2_size.to_le_bytes());

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0).round() as i16;
        wav_data.extend_from_slice(&sample_i16.to_le_bytes());
    }

    let mut f = File::create(path).expect("create wav");
    f.write_all(&wav_data).expect("write wav");
    println!(
        "Wrote {} samples ({:.2}s) to {}",
        samples.len(),
        samples.len() as f32 / 44100.0,
        path
    );
}

fn analyze_samples(label: &str, samples: &[f32]) {
    if samples.is_empty() {
        println!("[{}] Empty sample buffer!", label);
        return;
    }
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut pops = Vec::new();
    let mut prev = samples[0];

    for (idx, &s) in samples.iter().enumerate() {
        if s < min {
            min = s;
        }
        if s > max {
            max = s;
        }
        sum += s as f64;
        sum_sq += (s as f64) * (s as f64);

        let delta = (s - prev).abs();
        if delta > 0.35 && idx > 0 {
            pops.push((idx, delta, prev, s));
        }
        prev = s;
    }

    let n = samples.len() as f64;
    let mean = sum / n;
    let rms = (sum_sq / n).sqrt();

    println!("=== Audio Analysis: {} ===", label);
    println!("  Total samples: {}", samples.len());
    println!("  Min / Max:     {:.3} / {:.3}", min, max);
    println!("  DC Offset:     {:.4}", mean);
    println!("  RMS Amplitude: {:.3}", rms);
    println!("  Pops (>0.35):  {}", pops.len());
    for (i, (idx, delta, p, c)) in pops.iter().take(10).enumerate() {
        println!(
            "    Pop #{}: idx={}, delta={:.3} (prev={:.3} -> curr={:.3})",
            i + 1,
            idx,
            delta,
            p,
            c
        );
    }
    if pops.len() > 10 {
        println!("    ... and {} more pops", pops.len() - 10);
    }
}

#[test]
fn test_zelda_audio_diagnostics() {
    let rom_path = match std::env::var("ROM_PATH").or_else(|_| std::env::var("NES_ROM")) {
        Ok(path) => path,
        Err(_) => {
            println!("Skipping test: ROM_PATH or NES_ROM environment variable not set");
            return;
        }
    };
    if !Path::new(&rom_path).exists() {
        println!("Skipping test, ROM not found at {}", rom_path);
        return;
    }

    let cartridge = Cartridge::from_file(&rom_path).expect("load cart");
    let mut nes = Nes::new(cartridge);

    let mut title_samples = Vec::new();

    // Step 400 frames of title screen
    for _ in 0..400 {
        nes.step_frame();
        if let Ok(mut buf) = nes.bus.apu.sample_buffer.lock() {
            title_samples.extend(buf.drain(..));
        }
    }

    write_wav(&title_samples, "audio_dumps/zelda_title.wav");
    analyze_samples("Zelda Title Screen", &title_samples);

    // Start -> File Select
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);
    for _ in 0..60 {
        nes.step_frame();
    }

    // Register Name
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);
    for _ in 0..60 {
        nes.step_frame();
    }

    // Select Slot 1
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);
    for _ in 0..30 {
        nes.step_frame();
    }

    // Enter name 'A'
    nes.set_button_p1(JoypadButton::A, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::A, false);
    for _ in 0..30 {
        nes.step_frame();
    }

    // Select 3 times to END
    for _ in 0..3 {
        nes.set_button_p1(JoypadButton::Select, true);
        for _ in 0..8 {
            nes.step_frame();
        }
        nes.set_button_p1(JoypadButton::Select, false);
        for _ in 0..30 {
            nes.step_frame();
        }
    }

    // Start on END
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);
    for _ in 0..60 {
        nes.step_frame();
    }

    // Start to begin game
    nes.set_button_p1(JoypadButton::Start, true);
    for _ in 0..8 {
        nes.step_frame();
    }
    nes.set_button_p1(JoypadButton::Start, false);

    // Clear buffer and record overworld music
    if let Ok(mut buf) = nes.bus.apu.sample_buffer.lock() {
        buf.clear();
    }

    let mut overworld_samples = Vec::new();
    for _ in 0..300 {
        nes.step_frame();
        if let Ok(mut buf) = nes.bus.apu.sample_buffer.lock() {
            overworld_samples.extend(buf.drain(..));
        }
    }

    write_wav(&overworld_samples, "audio_dumps/zelda_overworld.wav");
    analyze_samples("Zelda Overworld", &overworld_samples);
}
