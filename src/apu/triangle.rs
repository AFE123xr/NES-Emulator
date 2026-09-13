use super::length_counter::LengthCounter;

pub static TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

pub struct Triangle {
    pub timer_period: u16,
    pub timer_value: u16,
    pub step_index: u8,

    pub length_counter: LengthCounter,

    // Linear counter
    pub linear_counter: u8,
    pub linear_reload_value: u8,
    pub linear_reload_flag: bool,
    pub control_flag: bool,
}

impl Default for Triangle {
    fn default() -> Self {
        Self::new()
    }
}

impl Triangle {
    pub fn new() -> Self {
        Self {
            timer_period: 0,
            timer_value: 0,
            step_index: 0,
            length_counter: LengthCounter::new(),
            linear_counter: 0,
            linear_reload_value: 0,
            linear_reload_flag: false,
            control_flag: false,
        }
    }

    pub fn write_linear_counter(&mut self, data: u8) {
        self.control_flag = (data & 0x80) != 0;
        self.length_counter.halt = self.control_flag;
        self.linear_reload_value = data & 0x7F;
    }

    pub fn write_timer_low(&mut self, data: u8) {
        self.timer_period = (self.timer_period & 0x0700) | (data as u16);
    }

    pub fn write_timer_high(&mut self, data: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | (((data as u16) & 0x07) << 8);
        self.length_counter.load_value(data);
        self.linear_reload_flag = true;
    }

    pub fn step_linear_counter(&mut self) {
        if self.linear_reload_flag {
            self.linear_counter = self.linear_reload_value;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }

        if !self.control_flag {
            self.linear_reload_flag = false;
        }
    }

    pub fn step_timer(&mut self) {
        if self.timer_value == 0 {
            self.timer_value = self.timer_period;
            if self.length_counter.counter > 0 && self.linear_counter > 0 && self.timer_period >= 2
            {
                self.step_index = (self.step_index + 1) % 32;
            }
        } else {
            self.timer_value -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        TRIANGLE_TABLE[self.step_index as usize]
    }
}
