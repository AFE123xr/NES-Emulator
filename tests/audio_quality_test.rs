//! Objective audio-quality diagnostics for the APU output path.
//!
//! These tests measure properties that directly affect perceived audio quality:
//! - **Aliasing**: inharmonic energy produced when the 1.79 MHz APU signal is
//!   decimated to the output sample rate without proper band-limiting.
//! - **Clipping**: how often the final output hits the hard [-1, 1] limit.
//!
//! Run with `cargo test --release --test audio_quality_test -- --nocapture`
//! to see the measured numbers.

use nes::apu::Apu;
use std::f64::consts::PI;

const OUT_RATE: f64 = 44_100.0;
const CPU_HZ: f64 = 1_789_773.0;

/// Generate `n` output samples from a single pulse channel playing a constant tone.
fn render_pulse_tone(timer: u16, duty: u8, n: usize) -> Vec<f32> {
    let mut apu = Apu::new();
    apu.write_register(0x4015, 0x01);
    // duty, length-halt, constant volume 15
    apu.write_register(0x4000, (duty << 6) | 0x30 | 0x0F);
    apu.write_register(0x4001, 0x00);
    apu.write_register(0x4002, (timer & 0xFF) as u8);
    apu.write_register(0x4003, ((timer >> 8) & 0x07) as u8 | 0x08);

    let mut out = Vec::with_capacity(n + 4096);
    // Discard the first ~0.25 s so the DC-blocking filters settle.
    let settle = 11_025;
    while out.len() < n + settle {
        for _ in 0..1000 {
            apu.step();
        }
        apu.flush_samples();
        out.extend(apu.sample_buffer.lock().unwrap().drain(..));
    }
    out.drain(..settle);
    out.truncate(n);
    out
}

/// Ratio (in dB) of harmonic energy to inharmonic (alias) energy.
/// Higher is better. Uses a Blackman-Harris-windowed DFT over 60 Hz..16 kHz evaluated at every bin.
fn harmonic_to_alias_db(samples: &[f32], f0: f64) -> f64 {
    let n = samples.len();
    let window: Vec<f64> = (0..n)
        .map(|i| {
            // 4-term Blackman-Harris: ~-92 dB sidelobes, so window leakage does
            // not masquerade as aliasing.
            let x = 2.0 * PI * i as f64 / n as f64;
            0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos() - 0.01168 * (3.0 * x).cos()
        })
        .collect();
    let bin_hz = OUT_RATE / n as f64;

    let mut harmonic = 0.0f64;
    let mut alias = 0.0f64;
    // Skip the lowest bins (DC / high-pass region).
    for k in (60.0 / bin_hz) as usize..(16_000.0 / bin_hz) as usize {
        let w = 2.0 * PI * k as f64 / n as f64;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let (cw, sw) = (w.cos(), w.sin());
        // Rotating phasor to avoid sin/cos per sample.
        let (mut c, mut s) = (1.0f64, 0.0f64);
        for (x, win) in samples.iter().zip(&window) {
            let v = *x as f64 * win;
            re += v * c;
            im -= v * s;
            let nc = c * cw - s * sw;
            s = s * cw + c * sw;
            c = nc;
        }
        let p = re * re + im * im;
        let f = k as f64 * bin_hz;
        let h = (f / f0).round();
        if h >= 1.0 && (f - h * f0).abs() <= 5.0 * bin_hz {
            harmonic += p;
        } else {
            alias += p;
        }
    }
    10.0 * (harmonic / alias.max(1e-30)).log10()
}

#[test]
fn test_pulse_aliasing_is_low() {
    // A selection of high-ish notes where aliasing is most audible.
    let mut worst = f64::INFINITY;
    for &(timer, duty) in &[(63u16, 2u8), (40, 1), (100, 0), (28, 2)] {
        let f0 = CPU_HZ / (16.0 * (timer as f64 + 1.0));
        let s = render_pulse_tone(timer, duty, 4096);
        let db = harmonic_to_alias_db(&s, f0);
        println!(
            "pulse timer={:>3} duty={} f0={:>7.1} Hz  harmonic/alias = {:>5.1} dB",
            timer, duty, f0, db
        );
        worst = worst.min(db);
    }
    println!("worst harmonic/alias ratio: {:.1} dB", worst);
    assert!(
        worst > 45.0,
        "Aliasing too high: worst harmonic/alias ratio {:.1} dB",
        worst
    );
}

#[test]
fn test_full_scale_output_does_not_clip() {
    // Worst-case: both pulses + triangle + noise at max, plus DMC at max level.
    let mut apu = Apu::new();
    apu.write_register(0x4015, 0x0F);
    apu.write_register(0x4000, 0xBF);
    apu.write_register(0x4002, 0x80);
    apu.write_register(0x4003, 0x08);
    apu.write_register(0x4004, 0xBF);
    apu.write_register(0x4006, 0x60);
    apu.write_register(0x4007, 0x08);
    apu.write_register(0x4008, 0xFF);
    apu.write_register(0x400A, 0x80);
    apu.write_register(0x400B, 0x08);
    apu.write_register(0x400C, 0x3F);
    apu.write_register(0x400E, 0x04);
    apu.write_register(0x400F, 0x08);

    let mut samples = Vec::new();
    for frame in 0..120 {
        // Toggle DMC direct-load between extremes (worst-case step).
        apu.write_register(0x4011, if frame % 20 < 10 { 0x7F } else { 0x00 });
        for _ in 0..29_780 {
            apu.step();
        }
        apu.flush_samples();
        samples.extend(apu.sample_buffer.lock().unwrap().drain(..));
    }
    // Count samples where the soft limiter (knee at 0.8) is engaged.
    let clipped = samples.iter().filter(|s| s.abs() > 0.8).count();
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let pct = 100.0 * clipped as f64 / samples.len() as f64;
    println!("full-scale peak = {:.3}, limited = {:.2}%", peak, pct);
    assert!(pct < 0.5, "Output hits the limiter {:.2}% of the time", pct);
}

/// Level check against a real game (set `ROM_PATH`). Presses Start to get past
/// the title screen and reports peak / RMS / limiter usage.
#[test]
fn test_rom_output_levels() {
    let rom_path = match std::env::var("ROM_PATH") {
        Ok(p) if std::path::Path::new(&p).exists() => p,
        _ => {
            println!("Skipping: ROM_PATH not set");
            return;
        }
    };
    let cart = nes::Cartridge::from_file(&rom_path).expect("load cart");
    let mut nes = nes::Nes::new(cart);
    let mut samples = Vec::new();
    for frame in 0..1800 {
        let press = (200..210).contains(&frame) || (400..410).contains(&frame);
        nes.set_button_p1(nes::JoypadButton::Start, press);
        nes.step_frame();
        samples.extend(nes.bus.apu.sample_buffer.lock().unwrap().drain(..));
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let rms =
        (samples.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();
    let limited = samples.iter().filter(|s| s.abs() > 0.8).count();
    println!(
        "{}: peak={:.3} rms={:.3} limited={:.3}%",
        rom_path,
        peak,
        rms,
        100.0 * limited as f64 / samples.len() as f64
    );
}
