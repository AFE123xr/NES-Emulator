pub static LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

pub struct LengthCounter {
    pub counter: u8,
    pub halt: bool,
    pub enabled: bool,
}

impl Default for LengthCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl LengthCounter {
    pub fn new() -> Self {
        Self {
            counter: 0,
            halt: false,
            enabled: false,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.counter = 0;
        }
    }

    pub fn load_value(&mut self, index: u8) {
        if self.enabled {
            self.counter = LENGTH_TABLE[(index >> 3) as usize];
        }
    }

    pub fn step(&mut self) {
        if self.counter > 0 && !self.halt {
            self.counter -= 1;
        }
    }
}
