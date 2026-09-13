use super::envelope::Envelope;
use super::length_counter::LengthCounter;

pub static DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
    [0, 1, 1, 0, 0, 0, 0, 0], // 25%
    [0, 1, 1, 1, 1, 0, 0, 0], // 50%
    [1, 0, 0, 1, 1, 1, 1, 1], // 75%
];

pub struct Pulse {
    pub duty_mode: u8,
    pub duty_step: u8,
    pub timer_period: u16,
    pub timer_value: u16,

    pub envelope: Envelope,
    pub length_counter: LengthCounter,

    // Sweep
    pub sweep_enabled: bool,
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,
    pub sweep_divider: u8,
    pub sweep_reload: bool,

    pub is_channel_2: bool,
}

impl Pulse {
    pub fn new(is_channel_2: bool) -> Self {
        Self {
            duty_mode: 0,
            duty_step: 0,
            timer_period: 0,
            timer_value: 0,
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_divider: 0,
            sweep_reload: false,
            is_channel_2,
        }
    }

    pub fn write_ctrl(&mut self, data: u8) {
        self.duty_mode = data >> 6;
        let halt = (data & 0x20) != 0;
        self.length_counter.halt = halt;
        self.envelope.loop_flag = halt;
        self.envelope.constant_volume = (data & 0x10) != 0;
        self.envelope.volume_or_period = data & 0x0F;
    }

    pub fn write_sweep(&mut self, data: u8) {
        self.sweep_enabled = (data & 0x80) != 0;
        self.sweep_period = (data >> 4) & 0x07;
        self.sweep_negate = (data & 0x08) != 0;
        self.sweep_shift = data & 0x07;
        self.sweep_reload = true;
    }

    pub fn write_timer_low(&mut self, data: u8) {
        self.timer_period = (self.timer_period & 0x0700) | (data as u16);
    }

    pub fn write_timer_high(&mut self, data: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | (((data as u16) & 0x07) << 8);
        self.length_counter.load_value(data);
        self.envelope.start_flag = true;
        self.duty_step = 0;
    }

    fn target_period(&self) -> u16 {
        let change = self.timer_period >> self.sweep_shift;
        if self.sweep_negate {
            if self.is_channel_2 {
                self.timer_period.saturating_sub(change)
            } else {
                self.timer_period.saturating_sub(change).saturating_sub(1)
            }
        } else {
            self.timer_period.saturating_add(change)
        }
    }

    pub fn is_muting(&self) -> bool {
        self.timer_period < 8 || self.target_period() > 0x07FF
    }

    pub fn step_timer(&mut self) {
        if self.timer_value == 0 {
            self.timer_value = self.timer_period;
            self.duty_step = (self.duty_step + 1) % 8;
        } else {
            self.timer_value -= 1;
        }
    }

    pub fn step_sweep(&mut self) {
        let target = self.target_period();
        if self.sweep_divider == 0
            && self.sweep_enabled
            && self.sweep_shift > 0
            && !self.is_muting()
        {
            self.timer_period = target;
        }

        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        if !self.length_counter.enabled || self.length_counter.counter == 0 || self.is_muting() {
            return 0;
        }

        if DUTY_TABLE[self.duty_mode as usize][self.duty_step as usize] == 0 {
            return 0;
        }

        self.envelope.output_volume()
    }
}
