pub mod dmc;
pub mod envelope;
pub mod filter;
pub mod length_counter;
pub mod noise;
pub mod pulse;
pub mod triangle;

use dmc::Dmc;
use filter::FirstOrderFilter;
use noise::Noise;
use pulse::Pulse;
use std::sync::{Arc, Mutex};
use triangle::Triangle;

pub const CPU_CLOCK_HZ: f32 = 1_789_773.0;
pub const SAMPLE_RATE: f32 = 44_100.0;

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
    sample_timer: f32,
    sample_period: f32,
    pub sample_buffer: Arc<Mutex<Vec<f32>>>,
    local_samples: Vec<f32>,

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
            sample_timer: 0.0,
            sample_period: CPU_CLOCK_HZ / SAMPLE_RATE,
            sample_buffer: Arc::new(Mutex::new(Vec::with_capacity(4096))),
            local_samples: Vec::with_capacity(128),
            filter_hp1: FirstOrderFilter::high_pass(SAMPLE_RATE, 90.0),
            filter_hp2: FirstOrderFilter::high_pass(SAMPLE_RATE, 440.0),
            filter_lp: FirstOrderFilter::low_pass(SAMPLE_RATE, 14_000.0),
        }
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

        // Audio sampling at 44.1 kHz
        self.sample_timer += 1.0;
        if self.sample_timer >= self.sample_period {
            self.sample_timer -= self.sample_period;
            let sample = self.generate_sample();
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

    fn generate_sample(&mut self) -> f32 {
        let p1 = self.pulse1.output() as f32;
        let p2 = self.pulse2.output() as f32;
        let pulse_out = if p1 + p2 > 0.0 {
            95.88 / ((8128.0 / (p1 + p2)) + 100.0)
        } else {
            0.0
        };

        let tri = self.triangle.output() as f32;
        let noise = self.noise.output() as f32;
        let dmc = self.dmc.output() as f32;
        let tnd_denom = (tri / 8227.0) + (noise / 12241.0) + (dmc / 22638.0);
        let tnd_out = if tnd_denom > 0.0 {
            159.79 / ((1.0 / tnd_denom) + 100.0)
        } else {
            0.0
        };

        let raw_sample = pulse_out + tnd_out;

        // Apply filters
        let s = self.filter_lp.process(raw_sample);
        let s = self.filter_hp1.process(s);
        let s = self.filter_hp2.process(s);

        // Normalize: High-pass filters already centered the audio around 0.0.
        (s * 2.5).clamp(-1.0, 1.0)
    }
}
