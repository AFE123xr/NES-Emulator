pub struct Envelope {
    pub start_flag: bool,
    pub divider: u8,
    pub decay_level: u8,
    pub loop_flag: bool,
    pub constant_volume: bool,
    pub volume_or_period: u8,
}

impl Default for Envelope {
    fn default() -> Self {
        Self::new()
    }
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            start_flag: false,
            divider: 0,
            decay_level: 0,
            loop_flag: false,
            constant_volume: false,
            volume_or_period: 0,
        }
    }

    pub fn output_volume(&self) -> u8 {
        if self.constant_volume {
            self.volume_or_period
        } else {
            self.decay_level
        }
    }

    pub fn step(&mut self) {
        if self.start_flag {
            self.start_flag = false;
            self.decay_level = 15;
            self.divider = self.volume_or_period;
        } else if self.divider == 0 {
            self.divider = self.volume_or_period;
            if self.decay_level > 0 {
                self.decay_level -= 1;
            } else if self.loop_flag {
                self.decay_level = 15;
            }
        } else {
            self.divider -= 1;
        }
    }
}
