pub mod diagnostics;

pub use diagnostics::{write_wav_file, AudioDiagnostics};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::AudioSubsystem;
use std::sync::{Arc, Mutex};

/// Buffer level (in samples) the rate controller steers towards (~56 ms).
const TARGET_BUFFER: f32 = 2500.0;
/// Maximum playback-rate deviation used to steer the buffer level (+/-0.5%,
/// about 8.6 cents — imperceptible, unlike dropping/duplicating samples).
const MAX_RATE_ADJUST: f64 = 0.005;

pub struct NesAudioCallback {
    pub sample_buffer: Arc<Mutex<Vec<f32>>>,
    pub muted: Arc<Mutex<bool>>,
    pub diagnostics: Arc<AudioDiagnostics>,
    last_sample: f32,
    prebuffered: bool,
    /// Fractional read position between `buf[0]` and `buf[1]`.
    frac: f64,
    /// Sample immediately preceding `buf[0]` (for cubic interpolation).
    prev: f32,
    /// Smoothed buffer level used by the rate controller.
    smoothed_level: f32,
}

/// 4-point, 3rd-order Hermite interpolation between `x0` and `x1`.
#[inline]
fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

impl AudioCallback for NesAudioCallback {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let is_muted = *self.muted.lock().unwrap_or_else(|e| e.into_inner());
        let mut buf = self.sample_buffer.lock().unwrap_or_else(|e| e.into_inner());

        let initial_buf_len = buf.len();
        self.diagnostics.update_buffer_level(initial_buf_len);

        // Pre-buffering: wait until buffer has ~2500 samples (~56ms) before starting audio
        if !self.prebuffered {
            if initial_buf_len >= TARGET_BUFFER as usize {
                self.prebuffered = true;
                self.smoothed_level = initial_buf_len as f32;
                self.frac = 0.0;
            } else {
                for item in out.iter_mut() {
                    self.last_sample *= 0.92;
                    *item = self.last_sample;
                }
                self.diagnostics.record_samples(out);
                return;
            }
        }

        if is_muted {
            buf.drain(..out.len().min(initial_buf_len));
            for item in out.iter_mut() {
                *item = 0.0;
            }
            self.last_sample = 0.0;
            self.diagnostics.record_samples(out);
            return;
        }

        // Smooth rate control: nudge the playback ratio proportionally to how
        // far the (smoothed) buffer level is from the target, then resample
        // with cubic interpolation so there are no discontinuities.
        self.smoothed_level += 0.1 * (initial_buf_len as f32 - self.smoothed_level);
        let error = ((self.smoothed_level - TARGET_BUFFER) / TARGET_BUFFER) as f64;
        let ratio = 1.0 + (error * 0.01).clamp(-MAX_RATE_ADJUST, MAX_RATE_ADJUST);

        let mut idx = 0usize;
        let mut samples_written = 0;
        // Need buf[idx + 2] for the interpolator's look-ahead.
        while samples_written < out.len() && idx + 2 < buf.len() {
            let xm1 = if idx == 0 { self.prev } else { buf[idx - 1] };
            let sample = hermite(xm1, buf[idx], buf[idx + 1], buf[idx + 2], self.frac as f32);

            // Check for discontinuity pops (> 0.35 jump)
            let delta = (sample - self.last_sample).abs();
            if delta > 0.35 && samples_written > 0 {
                self.diagnostics
                    .record_pop(delta, self.last_sample, sample, samples_written);
            }

            out[samples_written] = sample;
            self.last_sample = sample;
            samples_written += 1;

            self.frac += ratio;
            let whole = self.frac.floor();
            idx += whole as usize;
            self.frac -= whole;
        }

        let consumed = idx.min(buf.len());
        if consumed > 0 {
            self.prev = buf[consumed - 1];
        }
        buf.drain(..consumed);

        // Handle starvation / buffer underrun gracefully
        if samples_written < out.len() {
            let starved_count = out.len() - samples_written;
            self.diagnostics.record_underrun(starved_count, buf.len());

            // Smooth fade-out to 0.0 instead of instantaneous cliff to prevent audible pop
            for item in out.iter_mut().skip(samples_written) {
                self.last_sample *= 0.92;
                *item = self.last_sample;
            }
            // Re-prime the buffer before resuming so playback doesn't stutter.
            self.prebuffered = false;
        }

        // Record audio samples for WAV export and diagnostics
        self.diagnostics.record_samples(out);
    }
}

pub struct AudioPlayer {
    _device: Option<AudioDevice<NesAudioCallback>>,
    pub muted: Arc<Mutex<bool>>,
    pub diagnostics: Arc<AudioDiagnostics>,
}

impl AudioPlayer {
    pub fn new(
        audio_subsystem: &AudioSubsystem,
        sample_buffer: Arc<Mutex<Vec<f32>>>,
        debug_audio: bool,
    ) -> Self {
        let muted = Arc::new(Mutex::new(false));
        let diagnostics = Arc::new(AudioDiagnostics::new(debug_audio));

        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1), // Mono
            samples: Some(512),
        };

        let muted_clone = Arc::clone(&muted);
        let diag_clone = Arc::clone(&diagnostics);
        let device = audio_subsystem.open_playback(None, &desired_spec, |_spec| NesAudioCallback {
            sample_buffer,
            muted: muted_clone,
            diagnostics: diag_clone,
            last_sample: 0.0,
            prebuffered: false,
            frac: 0.0,
            prev: 0.0,
            smoothed_level: TARGET_BUFFER,
        });

        match device {
            Ok(dev) => {
                dev.resume();
                Self {
                    _device: Some(dev),
                    muted,
                    diagnostics,
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to initialize audio playback: {}", e);
                Self {
                    _device: None,
                    muted,
                    diagnostics,
                }
            }
        }
    }

    pub fn toggle_mute(&self) {
        if let Ok(mut m) = self.muted.lock() {
            *m = !*m;
            println!("Audio {}", if *m { "muted" } else { "unmuted" });
        }
    }

    pub fn toggle_diagnostics(&self) {
        self.diagnostics.toggle();
    }

    pub fn save_wav(&self, rom_title: &str) -> Result<String, String> {
        self.diagnostics.save_wav(rom_title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_callback(buf: Arc<Mutex<Vec<f32>>>) -> NesAudioCallback {
        NesAudioCallback {
            sample_buffer: buf,
            muted: Arc::new(Mutex::new(false)),
            diagnostics: Arc::new(AudioDiagnostics::new(false)),
            last_sample: 0.0,
            prebuffered: false,
            frac: 0.0,
            prev: 0.0,
            smoothed_level: TARGET_BUFFER,
        }
    }

    /// A smooth sine fed through the rate-controlled resampler (with the
    /// buffer deliberately above target, forcing rate adjustment) must come
    /// out without any sample-to-sample discontinuities.
    #[test]
    fn rate_control_is_click_free() {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let mut cb = make_callback(Arc::clone(&buf));
        let step = 2.0 * std::f32::consts::PI * 1000.0 / 44_100.0;
        let mut phase = 0.0f32;
        let mut out = vec![0.0f32; 512];
        let mut max_delta = 0.0f32;
        let mut prev: Option<f32> = None;

        for i in 0..400 {
            // Produce slightly more than consumed so the buffer sits high.
            let produce = if i == 0 { 4000 } else { 515 };
            {
                let mut b = buf.lock().unwrap();
                for _ in 0..produce {
                    b.push(0.5 * phase.sin());
                    phase += step;
                }
            }
            cb.callback(&mut out);
            for &s in &out {
                if let Some(p) = prev {
                    max_delta = max_delta.max((s - p).abs());
                }
                prev = Some(s);
            }
        }
        // Max slope of 0.5*sin at 1 kHz is ~0.071 per sample.
        assert!(max_delta < 0.08, "discontinuity detected: {}", max_delta);
        // Rate control must keep the backlog bounded despite overproduction.
        assert!(buf.lock().unwrap().len() < 6000);
    }
}
