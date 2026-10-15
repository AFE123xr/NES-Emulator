pub mod dmc;
pub mod envelope;
pub mod filter;
pub mod length_counter;
pub mod noise;
pub mod pulse;
pub mod resampler;
pub mod triangle;

use dmc::Dmc;
use filter::FirstOrderFilter;
use noise::Noise;
use pulse::Pulse;
use resampler::Decimator;
use std::sync::{Arc, Mutex};
use triangle::Triangle;

pub const CPU_CLOCK_HZ: f32 = 1_789_773.0;
pub const SAMPLE_RATE: f32 = 44_100.0;

/// Output gain applied after DC removal. The mixer's full-scale output is
/// ~1.0, and after the high-pass filters typical game music sits well within
/// +/-0.3, so this leaves headroom for loud DMC/noise passages.
const OUTPUT_GAIN: f32 = 2.0;

pub struct Apu {
    pub pulse1: Pulse,
    pub pulse2: Pulse,
    pub triangle: Triangle,
    pub noise: Noise,
    pub dmc: Dmc,

    // Frame counter
    pub frame_counter_mode: u8,
    pub irq_inhibit: bool,
    pub frame_irq: bool,
    pub cycle: usize,
    pub frame_seq_cycle: usize,

    // Audio sampling
    sample_rate: f32,
    decimator: Decimator,
    pub sample_buffer: Arc<Mutex<Vec<f32>>>,
    local_samples: Vec<f32>,

    // Nonlinear mixer lookup tables (nesdev "lookup table" mixer)
    pulse_table: [f32; 31],
    tnd_table: [f32; 203],

    // Filters
    filter_hp1: FirstOrderFilter,
    filter_hp2: FirstOrderFilter,
    filter_lp: FirstOrderFilter,
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    pub fn new() -> Self {
        let mut pulse_table = [0.0f32; 31];
        for (n, v) in pulse_table.iter_mut().enumerate().skip(1) {
            *v = 95.52 / (8128.0 / n as f32 + 100.0);
        }
        let mut tnd_table = [0.0f32; 203];
        for (n, v) in tnd_table.iter_mut().enumerate().skip(1) {
            *v = 163.67 / (24329.0 / n as f32 + 100.0);
        }

        Self {
            pulse1: Pulse::new(false),
            pulse2: Pulse::new(true),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            frame_counter_mode: 0,
            irq_inhibit: false,
            frame_irq: false,
            cycle: 0,
            frame_seq_cycle: 0,
            sample_rate: SAMPLE_RATE,
            decimator: Decimator::new(CPU_CLOCK_HZ, SAMPLE_RATE),
            sample_buffer: Arc::new(Mutex::new(Vec::with_capacity(4096))),
            local_samples: Vec::with_capacity(128),
            pulse_table,
            tnd_table,
            filter_hp1: FirstOrderFilter::high_pass(SAMPLE_RATE, 90.0),
            filter_hp2: FirstOrderFilter::high_pass(SAMPLE_RATE, 440.0),
            filter_lp: FirstOrderFilter::low_pass(SAMPLE_RATE, 14_000.0),
        }
    }

    /// Output sample rate currently being generated.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Reconfigure the output sample rate (e.g. to match the rate the audio
    /// device actually opened at). Resets the resampler and output filters.
    pub fn set_sample_rate(&mut self, rate: f32) {
        if rate <= 0.0 || (rate - self.sample_rate).abs() < f32::EPSILON {
            return;
        }
        self.sample_rate = rate;
        self.decimator = Decimator::new(CPU_CLOCK_HZ, rate);
        self.filter_hp1 = FirstOrderFilter::high_pass(rate, 90.0);
        self.filter_hp2 = FirstOrderFilter::high_pass(rate, 440.0);
        self.filter_lp = FirstOrderFilter::low_pass(rate, 14_000.0);
    }

    pub fn reset(&mut self) {
        self.write_status(0);
        self.frame_irq = false;
        self.cycle = 0;
        self.frame_seq_cycle = 0;
        self.local_samples.clear();
        if let Ok(mut buf) = self.sample_buffer.lock() {
            buf.clear();
        }
    }

    pub fn write_register(&mut self, addr: u16, data: u8) {
        match addr {
            0x4000 => self.pulse1.write_ctrl(data),
            0x4001 => self.pulse1.write_sweep(data),
            0x4002 => self.pulse1.write_timer_low(data),
            0x4003 => self.pulse1.write_timer_high(data),

            0x4004 => self.pulse2.write_ctrl(data),
            0x4005 => self.pulse2.write_sweep(data),
            0x4006 => self.pulse2.write_timer_low(data),
            0x4007 => self.pulse2.write_timer_high(data),

            0x4008 => self.triangle.write_linear_counter(data),
            0x400A => self.triangle.write_timer_low(data),
            0x400B => self.triangle.write_timer_high(data),

            0x400C => self.noise.write_ctrl(data),
            0x400E => self.noise.write_period(data),
            0x400F => self.noise.write_length(data),

            0x4010 => self.dmc.write_ctrl(data),
            0x4011 => self.dmc.write_direct_load(data),
            0x4012 => self.dmc.write_sample_address(data),
            0x4013 => self.dmc.write_sample_length(data),

            0x4015 => self.write_status(data),
            0x4017 => self.write_frame_counter(data),
            _ => {}
        }
    }

    pub fn read_status(&mut self) -> u8 {
        let mut status = 0;
        if self.pulse1.length_counter.counter > 0 {
            status |= 1 << 0;
        }
        if self.pulse2.length_counter.counter > 0 {
            status |= 1 << 1;
        }
        if self.triangle.length_counter.counter > 0 {
            status |= 1 << 2;
        }
        if self.noise.length_counter.counter > 0 {
            status |= 1 << 3;
        }
        if self.dmc.bytes_remaining > 0 {
            status |= 1 << 4;
        }
        if self.frame_irq {
            status |= 1 << 6;
        }
        if self.dmc.irq_active {
            status |= 1 << 7;
        }

        self.frame_irq = false;
        status
    }

    pub fn write_status(&mut self, data: u8) {
        self.pulse1.length_counter.set_enabled((data & 0x01) != 0);
        self.pulse2.length_counter.set_enabled((data & 0x02) != 0);
        self.triangle.length_counter.set_enabled((data & 0x04) != 0);
        self.noise.length_counter.set_enabled((data & 0x08) != 0);
        self.dmc.set_enabled((data & 0x10) != 0);
    }

    pub fn write_frame_counter(&mut self, data: u8) {
        self.frame_counter_mode = data >> 7;
        self.irq_inhibit = (data & 0x40) != 0;
        if self.irq_inhibit {
            self.frame_irq = false;
        }
        self.cycle = 0;
        self.frame_seq_cycle = 0;
        if self.frame_counter_mode == 1 {
            self.step_quarter_frame();
            self.step_half_frame();
        }
    }

    pub fn irq_state(&self) -> bool {
        self.frame_irq || self.dmc.irq_active
    }

    fn step_quarter_frame(&mut self) {
        self.pulse1.envelope.step();
        self.pulse2.envelope.step();
        self.triangle.step_linear_counter();
        self.noise.envelope.step();
    }

    fn step_half_frame(&mut self) {
        self.pulse1.length_counter.step();
        self.pulse1.step_sweep();

        self.pulse2.length_counter.step();
        self.pulse2.step_sweep();

        self.triangle.length_counter.step();
        self.noise.length_counter.step();
    }

    pub fn step(&mut self) {
        // Triangle, noise, and DMC timers step every CPU cycle
        self.triangle.step_timer();
        self.noise.step_timer();
        self.dmc.step_timer();

        // APU cycles occur every 2 CPU cycles
        if (self.cycle & 1) == 0 {
            self.pulse1.step_timer();
            self.pulse2.step_timer();

            // Frame sequencer stepping (clocked on APU cycles)
            self.frame_seq_cycle += 1;
            if self.frame_counter_mode == 0 {
                // 4-step sequence
                match self.frame_seq_cycle {
                    3729 => self.step_quarter_frame(),
                    7457 => {
                        self.step_quarter_frame();
                        self.step_half_frame();
                    }
                    11186 => self.step_quarter_frame(),
                    14915 => {
                        self.step_quarter_frame();
                        self.step_half_frame();
                        if !self.irq_inhibit {
                            self.frame_irq = true;
                        }
                        self.frame_seq_cycle = 0;
                    }
                    _ => {}
                }
            } else {
                // 5-step sequence
                match self.frame_seq_cycle {
                    3729 => self.step_quarter_frame(),
                    7457 => {
                        self.step_quarter_frame();
                        self.step_half_frame();
                    }
                    11186 => self.step_quarter_frame(),
                    18641 => {
                        self.step_quarter_frame();
                        self.step_half_frame();
                        self.frame_seq_cycle = 0;
                    }
                    _ => {}
                }
            }
        }

        self.cycle += 1;

        // Mix at the native CPU rate and let the band-limited decimator
        // produce output-rate samples. Point-sampling here would alias the
        // waveforms' ultrasonic harmonics back into the audible range.
        let mixed = self.mix();
        if let Some(sample) = self.decimator.push(mixed) {
            let sample = self.post_process(sample);
            self.local_samples.push(sample);

            if self.local_samples.len() >= 32 {
                self.flush_samples();
            }
        }
    }

    pub fn flush_samples(&mut self) {
        if self.local_samples.is_empty() {
            return;
        }
        if let Ok(mut buf) = self.sample_buffer.lock() {
            if buf.len() + self.local_samples.len() <= 8192 {
                buf.extend(self.local_samples.drain(..));
            } else {
                // Prevent excessive buffer backlog
                let drain_count = (buf.len() + self.local_samples.len()).saturating_sub(4096);
                let buf_len = buf.len();
                buf.drain(..drain_count.min(buf_len));
                buf.extend(self.local_samples.drain(..));
            }
        } else {
            self.local_samples.clear();
        }
    }

    /// Nonlinear NES mixer (lookup-table approximation), evaluated per CPU cycle.
    #[inline]
    fn mix(&self) -> f32 {
        let p = (self.pulse1.output() + self.pulse2.output()) as usize;
        let tnd = 3 * self.triangle.output() as usize
            + 2 * self.noise.output() as usize
            + self.dmc.output() as usize;
        self.pulse_table[p] + self.tnd_table[tnd]
    }

    /// Output-rate filtering (NES analog output stage), gain, and limiting.
    fn post_process(&mut self, raw_sample: f32) -> f32 {
        let s = self.filter_lp.process(raw_sample);
        let s = self.filter_hp1.process(s);
        let s = self.filter_hp2.process(s);
        soft_clip(s * OUTPUT_GAIN)
    }
}

/// Transparent below 0.8; smoothly saturates towards 1.0 above it, avoiding
/// the harsh distortion of a hard clamp on loud DMC/noise transients.
#[inline]
fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let over = (a - KNEE) / (1.0 - KNEE);
        (KNEE + (1.0 - KNEE) * over.tanh()).copysign(x)
    }
}
