//! Band-limited decimation from the APU's native CPU clock (~1.79 MHz) down to
//! the host audio sample rate.
//!
//! The APU produces hard-edged square/triangle/noise waveforms whose harmonics
//! extend far above the audible range. Simply picking one value every
//! `CPU_CLOCK / SAMPLE_RATE` cycles (point sampling) folds all of that energy
//! back into the audible band as inharmonic "aliasing" — the gritty, metallic,
//! out-of-tune overtones that make emulator audio sound harsh.
//!
//! This decimator works in two stages:
//! 1. **Box-filter** (running average) every CPU-cycle sample down to an
//!    intermediate rate of `OVERSAMPLE x output_rate`. The box filter has
//!    spectral nulls exactly at multiples of the intermediate rate, which is
//!    where the energy that would alias in stage 2 lives.
//! 2. **Windowed-sinc FIR** low-pass (Blackman window) at the intermediate
//!    rate, keeping only every `OVERSAMPLE`-th output. This removes everything
//!    between ~20 kHz and the intermediate Nyquist before the final decimation.

use std::f64::consts::PI;

/// Intermediate oversampling factor relative to the output sample rate.
const OVERSAMPLE: usize = 4;
/// Number of FIR taps (at the intermediate rate).
const TAPS: usize = 96;

pub struct Decimator {
    cycles_per_inter: f64,
    timer: f64,
    inv_cycles_per_inter: f32,
    acc: f32,

    taps: [f32; TAPS],
    /// History stored twice so the convolution can read a contiguous slice.
    history: [f32; TAPS * 2],
    pos: usize,
    phase: usize,
}

impl Decimator {
    pub fn new(cpu_clock_hz: f32, output_rate: f32) -> Self {
        let inter_rate = output_rate as f64 * OVERSAMPLE as f64;
        // Cutoff: keep the audible band, reach the stopband before output Nyquist
        // aliases fold back below ~20 kHz.
        let cutoff_hz = (0.41 * output_rate as f64).min(18_000.0);
        let fc = cutoff_hz / inter_rate; // normalized (cycles/sample)

        let mut taps = [0.0f32; TAPS];
        let m = (TAPS - 1) as f64;
        let mut sum = 0.0f64;
        let mut raw = [0.0f64; TAPS];
        for (i, r) in raw.iter_mut().enumerate() {
            let x = i as f64 - m / 2.0;
            let sinc = if x.abs() < 1e-9 {
                2.0 * fc
            } else {
                (2.0 * PI * fc * x).sin() / (PI * x)
            };
            let w = 0.42 - 0.5 * (2.0 * PI * i as f64 / m).cos()
                + 0.08 * (4.0 * PI * i as f64 / m).cos();
            *r = sinc * w;
            sum += *r;
        }
        for (t, r) in taps.iter_mut().zip(raw.iter()) {
            *t = (*r / sum) as f32;
        }

        Self {
            cycles_per_inter: cpu_clock_hz as f64 / inter_rate,
            inv_cycles_per_inter: (inter_rate / cpu_clock_hz as f64) as f32,
            timer: 0.0,
            acc: 0.0,
            taps,
            history: [0.0; TAPS * 2],
            pos: 0,
            phase: 0,
        }
    }

    /// Feed one CPU-cycle sample. Returns `Some(sample)` whenever a new
    /// output-rate sample is ready.
    #[inline]
    pub fn push(&mut self, input: f32) -> Option<f32> {
        self.timer += 1.0;
        if self.timer < self.cycles_per_inter {
            self.acc += input;
            return None;
        }
        // Split the boundary cycle between this window and the next so every
        // intermediate sample integrates exactly `cycles_per_inter` cycles.
        self.timer -= self.cycles_per_inter;
        let next_part = self.timer as f32;
        self.acc += input * (1.0 - next_part);

        let inter = self.acc * self.inv_cycles_per_inter;
        self.acc = input * next_part;

        self.history[self.pos] = inter;
        self.history[self.pos + TAPS] = inter;
        self.pos = (self.pos + 1) % TAPS;

        self.phase += 1;
        if self.phase < OVERSAMPLE {
            return None;
        }
        self.phase = 0;

        let window = &self.history[self.pos..self.pos + TAPS];
        let out = window
            .iter()
            .zip(self.taps.iter())
            .map(|(x, t)| x * t)
            .sum::<f32>();
        Some(out)
    }
}
