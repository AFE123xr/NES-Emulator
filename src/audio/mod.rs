pub mod diagnostics;

pub use diagnostics::{write_wav_file, AudioDiagnostics};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::AudioSubsystem;
use std::sync::{Arc, Mutex};

pub struct NesAudioCallback {
    pub sample_buffer: Arc<Mutex<Vec<f32>>>,
    pub muted: Arc<Mutex<bool>>,
    pub diagnostics: Arc<AudioDiagnostics>,
    last_sample: f32,
    prebuffered: bool,
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
            if initial_buf_len >= 2500 {
                self.prebuffered = true;
            } else {
                for item in out.iter_mut() {
                    *item = 0.0;
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

        // Dynamic Rate Control (DRC):
        // Target buffer: ~2500 samples (~56ms)
        // If buffer < 1800: duplicate 1 sample every 32 samples (~3% stretch)
        // If buffer > 3500: drop 1 sample every 32 samples (~3% compress)
        let stretch = initial_buf_len < 1800;
        let compress = initial_buf_len > 3500;

        let mut read_idx = 0;
        let mut samples_written = 0;
        let mut sample_counter = 0;

        while samples_written < out.len() && read_idx < buf.len() {
            let sample = buf[read_idx];
            read_idx += 1;
            sample_counter += 1;

            if compress && sample_counter % 32 == 0 && read_idx < buf.len() {
                // Drop next sample to compress
                read_idx += 1;
            }

            // Check for discontinuity pops (> 0.35 jump)
            let delta = (sample - self.last_sample).abs();
            if delta > 0.35 && samples_written > 0 {
                self.diagnostics
                    .record_pop(delta, self.last_sample, sample, samples_written);
            }

            out[samples_written] = sample;
            self.last_sample = sample;
            samples_written += 1;

            if stretch && sample_counter % 32 == 0 && samples_written < out.len() {
                // Duplicate current sample to stretch
                out[samples_written] = sample;
                samples_written += 1;
            }
        }

        buf.drain(..read_idx);

        // Handle starvation / buffer underrun gracefully
        if samples_written < out.len() {
            let starved_count = out.len() - samples_written;
            self.diagnostics.record_underrun(starved_count, buf.len());

            // Smooth fade-out to 0.0 instead of instantaneous cliff to prevent audible pop
            for item in out.iter_mut().skip(samples_written) {
                self.last_sample *= 0.92;
                *item = self.last_sample;
            }
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
