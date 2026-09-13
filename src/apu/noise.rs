use super::envelope::Envelope;
use super::length_counter::LengthCounter;

pub static NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

pub struct Noise {
    pub envelope: Envelope,
    pub length_counter: LengthCounter,

    pub mode: bool,
    pub shift_register: u16,
    pub timer_period: u16,
    pub timer_value: u16,
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

impl Noise {
    pub fn new() -> Self {
        Self {
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
            mode: false,
            shift_register: 1,
            timer_period: 0,
            timer_value: 0,
        }
    }

    pub fn write_ctrl(&mut self, data: u8) {
        let halt = (data & 0x20) != 0;
        self.length_counter.halt = halt;
        self.envelope.loop_flag = halt;
        self.envelope.constant_volume = (data & 0x10) != 0;
        self.envelope.volume_or_period = data & 0x0F;
    }

    pub fn write_period(&mut self, data: u8) {
        self.mode = (data & 0x80) != 0;
        self.timer_period = NOISE_PERIOD_TABLE[(data & 0x0F) as usize];
    }

    pub fn write_length(&mut self, data: u8) {
        self.length_counter.load_value(data);
        self.envelope.start_flag = true;
    }

    pub fn step_timer(&mut self) {
        if self.timer_value == 0 {
            self.timer_value = self.timer_period.saturating_sub(1);

            let bit0 = self.shift_register & 1;
            let other_bit = if self.mode {
                (self.shift_register >> 6) & 1
            } else {
                (self.shift_register >> 1) & 1
            };
            let feedback = bit0 ^ other_bit;
            self.shift_register = (self.shift_register >> 1) | (feedback << 14);
        } else {
            self.timer_value -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        if !self.length_counter.enabled || self.length_counter.counter == 0 {
            return 0;
        }

        if (self.shift_register & 1) != 0 {
            return 0;
        }

        self.envelope.output_volume()
    }
}
