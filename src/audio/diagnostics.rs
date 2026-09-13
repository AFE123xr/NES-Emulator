use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MAX_RECORDED_SAMPLES: usize = 44_100 * 120; // 2 minutes rolling buffer

fn epoch_to_timestamp(epoch_secs: u64) -> String {
    let secs_per_day = 86400;
    let mut days = (epoch_secs / secs_per_day) as i64;
    let rem_secs = (epoch_secs % secs_per_day) as u32;

    let hour = rem_secs / 3600;
    let min = (rem_secs % 3600) / 60;
    let sec = rem_secs % 60;

    // Convert days since Jan 1 1970 to YYYY-MM-DD
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1024 + doe / 1460 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        year, m, d, hour, min, sec
    )
}

pub fn write_wav_file<P: AsRef<Path>>(
    samples: &[f32],
    sample_rate: u32,
    path: P,
) -> Result<(), String> {
    let num_samples = samples.len() as u32;
    let subchunk2_size = num_samples * 2; // 16-bit mono = 2 bytes per sample
    let chunk_size = 36 + subchunk2_size;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;
    let bits_per_sample = 16u16;

    let mut wav_data = Vec::with_capacity(44 + subchunk2_size as usize);

    // RIFF header
    wav_data.extend_from_slice(b"RIFF");
    wav_data.extend_from_slice(&chunk_size.to_le_bytes());
    wav_data.extend_from_slice(b"WAVE");

    // fmt subchunk
    wav_data.extend_from_slice(b"fmt ");
    wav_data.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    wav_data.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 = PCM)
    wav_data.extend_from_slice(&1u16.to_le_bytes()); // NumChannels (1 = Mono)
    wav_data.extend_from_slice(&sample_rate.to_le_bytes());
    wav_data.extend_from_slice(&byte_rate.to_le_bytes());
    wav_data.extend_from_slice(&block_align.to_le_bytes());
    wav_data.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    wav_data.extend_from_slice(b"data");
    wav_data.extend_from_slice(&subchunk2_size.to_le_bytes());

    // 16-bit PCM little-endian samples
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0).round() as i16;
        wav_data.extend_from_slice(&sample_i16.to_le_bytes());
    }

    let p = path.as_ref();
    let mut file = File::create(p)
        .map_err(|e| format!("Failed to create WAV file '{}': {}", p.display(), e))?;
    file.write_all(&wav_data)
        .map_err(|e| format!("Failed to write WAV file '{}': {}", p.display(), e))?;

    Ok(())
}

pub struct AudioDiagnostics {
    pub enabled: AtomicBool,
    pub underrun_count: AtomicUsize,
    pub overflow_count: AtomicUsize,
    pub pop_count: AtomicUsize,
    pub total_starved_samples: AtomicUsize,
    pub max_starved_samples: AtomicUsize,
    pub current_buffer_level: AtomicUsize,
    pub min_buffer_level: AtomicUsize,
    pub max_buffer_level: AtomicUsize,
    last_report_time: Mutex<Instant>,
    pub recorded_samples: Mutex<VecDeque<f32>>,
}

impl Default for AudioDiagnostics {
    fn default() -> Self {
        Self::new(false)
    }
}

impl AudioDiagnostics {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            underrun_count: AtomicUsize::new(0),
            overflow_count: AtomicUsize::new(0),
            pop_count: AtomicUsize::new(0),
            total_starved_samples: AtomicUsize::new(0),
            max_starved_samples: AtomicUsize::new(0),
            current_buffer_level: AtomicUsize::new(0),
            min_buffer_level: AtomicUsize::new(usize::MAX),
            max_buffer_level: AtomicUsize::new(0),
            last_report_time: Mutex::new(Instant::now()),
            recorded_samples: Mutex::new(VecDeque::with_capacity(44_100 * 10)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::Relaxed);
        println!(
            "Audio diagnostics {}",
            if val {
                "ENABLED (real-time monitoring active)"
            } else {
                "DISABLED"
            }
        );
    }

    pub fn toggle(&self) {
        let current = self.is_enabled();
        self.set_enabled(!current);
    }

    pub fn record_underrun(&self, starved_samples: usize, buffer_len: usize) {
        let count = self.underrun_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.total_starved_samples
            .fetch_add(starved_samples, Ordering::Relaxed);

        let mut max = self.max_starved_samples.load(Ordering::Relaxed);
        while starved_samples > max {
            match self.max_starved_samples.compare_exchange_weak(
                max,
                starved_samples,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(curr) => max = curr,
            }
        }

        if self.is_enabled() {
            let starved_ms = (starved_samples as f64 / 44.1).round();
            eprintln!(
                "[AUDIO DIAG #{} - UNDERRUN] Buffer starved by {} samples ({:.1} ms)! Available: {} samples",
                count, starved_samples, starved_ms, buffer_len
            );
        }
    }

    pub fn record_overflow(&self, dropped_samples: usize) {
        let count = self.overflow_count.fetch_add(1, Ordering::Relaxed) + 1;
        if self.is_enabled() {
            eprintln!(
                "[AUDIO DIAG #{} - OVERFLOW] Buffer full! Dropped {} audio samples to prevent latency drift",
                count, dropped_samples
            );
        }
    }

    pub fn record_pop(&self, delta: f32, prev: f32, curr: f32, sample_idx: usize) {
        let count = self.pop_count.fetch_add(1, Ordering::Relaxed) + 1;
        if self.is_enabled() {
            eprintln!(
                "[AUDIO DIAG #{} - POP DETECTED] Delta: {:.3} at sample index {} (prev: {:.3}, curr: {:.3})",
                count, delta, sample_idx, prev, curr
            );
        }
    }

    pub fn update_buffer_level(&self, level: usize) {
        self.current_buffer_level.store(level, Ordering::Relaxed);

        let mut min = self.min_buffer_level.load(Ordering::Relaxed);
        while level < min {
            match self.min_buffer_level.compare_exchange_weak(
                min,
                level,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(curr) => min = curr,
            }
        }

        let mut max = self.max_buffer_level.load(Ordering::Relaxed);
        while level > max {
            match self.max_buffer_level.compare_exchange_weak(
                max,
                level,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(curr) => max = curr,
            }
        }

        if self.is_enabled() {
            let mut last_report = self.last_report_time.lock().unwrap();
            if last_report.elapsed().as_secs_f32() >= 1.0 {
                let curr_samples = self.current_buffer_level.load(Ordering::Relaxed);
                let min_samples = self.min_buffer_level.swap(usize::MAX, Ordering::Relaxed);
                let max_samples = self.max_buffer_level.swap(0, Ordering::Relaxed);
                let underruns = self.underrun_count.load(Ordering::Relaxed);
                let overflows = self.overflow_count.load(Ordering::Relaxed);
                let pops = self.pop_count.load(Ordering::Relaxed);

                let curr_ms = curr_samples as f32 / 44.1;
                let min_ms = if min_samples == usize::MAX {
                    0.0
                } else {
                    min_samples as f32 / 44.1
                };
                let max_ms = max_samples as f32 / 44.1;

                println!(
                    "[AUDIO DIAG STATUS] Buffer: {:4} smp ({:.1}ms) [min: {:.1}ms, max: {:.1}ms] | Underruns: {} | Overflows: {} | Pops: {}",
                    curr_samples, curr_ms, min_ms, max_ms, underruns, overflows, pops
                );
                *last_report = Instant::now();
            }
        }
    }

    pub fn print_summary(&self) {
        let underruns = self.underrun_count.load(Ordering::Relaxed);
        let overflows = self.overflow_count.load(Ordering::Relaxed);
        let pops = self.pop_count.load(Ordering::Relaxed);
        let total_starved = self.total_starved_samples.load(Ordering::Relaxed);
        let max_starved = self.max_starved_samples.load(Ordering::Relaxed);

        println!("=== Audio Diagnostic Summary ===");
        println!("  Underrun events:        {}", underruns);
        println!(
            "  Total starved samples:  {} ({:.1} ms)",
            total_starved,
            total_starved as f32 / 44.1
        );
        println!(
            "  Max single underrun:    {} ({:.1} ms)",
            max_starved,
            max_starved as f32 / 44.1
        );
        println!("  Overflow events:        {}", overflows);
        println!("  Detected pop anomalies: {}", pops);
        println!("================================");
    }

    pub fn record_samples(&self, samples: &[f32]) {
        if let Ok(mut buf) = self.recorded_samples.lock() {
            buf.extend(samples.iter().copied());
            if buf.len() > MAX_RECORDED_SAMPLES {
                let excess = buf.len() - MAX_RECORDED_SAMPLES;
                buf.drain(..excess);
            }
        }
    }

    pub fn sample_count(&self) -> usize {
        self.recorded_samples.lock().map(|b| b.len()).unwrap_or(0)
    }

    pub fn has_recorded_samples(&self) -> bool {
        self.sample_count() > 0
    }

    pub fn clear_recorded_samples(&self) {
        if let Ok(mut buf) = self.recorded_samples.lock() {
            buf.clear();
        }
    }

    pub fn save_wav(&self, rom_title: &str) -> Result<String, String> {
        let samples: Vec<f32> = {
            let buf = self
                .recorded_samples
                .lock()
                .map_err(|e| format!("Failed to lock recorded samples: {}", e))?;
            if buf.is_empty() {
                return Err("No recorded audio samples to save".to_string());
            }
            buf.iter().copied().collect()
        };

        let dir = Path::new("audio_dumps");
        if !dir.exists() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create audio_dumps directory: {}", e))?;
        }

        let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => epoch_to_timestamp(d.as_secs()),
            Err(_) => "audio".to_string(),
        };

        let clean_title: String = rom_title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        let filename = format!("audio_dumps/{}_{}.wav", clean_title, timestamp);
        write_wav_file(&samples, 44100, &filename)?;

        Ok(filename)
    }
}
