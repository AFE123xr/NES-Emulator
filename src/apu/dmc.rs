pub static DMC_RATE_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

pub struct Dmc {
    pub irq_enabled: bool,
    pub loop_flag: bool,
    pub irq_active: bool,

    pub rate_index: u8,
    pub timer_period: u16,
    pub timer_value: u16,

    pub output_level: u8,

    pub sample_address: u16,
    pub sample_length: u16,
    pub current_address: u16,
    pub bytes_remaining: u16,

    pub sample_buffer: Option<u8>,
    pub shift_register: u8,
    pub bits_remaining: u8,
    pub silence: bool,

    pub dma_needed: bool,
}

impl Default for Dmc {
    fn default() -> Self {
        Self::new()
    }
}

impl Dmc {
    pub fn new() -> Self {
        Self {
            irq_enabled: false,
            loop_flag: false,
            irq_active: false,
            rate_index: 0,
            timer_period: DMC_RATE_TABLE[0],
            timer_value: DMC_RATE_TABLE[0],
            output_level: 0,
            sample_address: 0xC000,
            sample_length: 1,
            current_address: 0xC000,
            bytes_remaining: 0,
            sample_buffer: None,
            shift_register: 0,
            bits_remaining: 8,
            silence: true,
            dma_needed: false,
        }
    }

    pub fn write_ctrl(&mut self, data: u8) {
        self.irq_enabled = (data & 0x80) != 0;
        self.loop_flag = (data & 0x40) != 0;
        if !self.irq_enabled {
            self.irq_active = false;
        }
        self.rate_index = data & 0x0F;
        self.timer_period = DMC_RATE_TABLE[self.rate_index as usize];
    }

    pub fn write_direct_load(&mut self, data: u8) {
        self.output_level = data & 0x7F;
    }

    pub fn write_sample_address(&mut self, data: u8) {
        self.sample_address = 0xC000 | ((data as u16) << 6);
    }

    pub fn write_sample_length(&mut self, data: u8) {
        self.sample_length = ((data as u16) << 4) | 1;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.bytes_remaining = 0;
        } else if self.bytes_remaining == 0 {
            self.restart();
        }
        self.irq_active = false;
    }

    fn restart(&mut self) {
        self.current_address = self.sample_address;
        self.bytes_remaining = self.sample_length;
        if self.sample_buffer.is_none() {
            self.dma_needed = true;
        }
    }

    pub fn load_sample_buffer(&mut self, byte: u8) {
        self.sample_buffer = Some(byte);
        self.dma_needed = false;
    }

    pub fn step_timer(&mut self) {
        if self.timer_value == 0 {
            self.timer_value = self.timer_period.saturating_sub(1);

            if !self.silence {
                if (self.shift_register & 1) != 0 {
                    if self.output_level <= 125 {
                        self.output_level += 2;
                    }
                } else if self.output_level >= 2 {
                    self.output_level -= 2;
                }
                self.shift_register >>= 1;
            }

            self.bits_remaining -= 1;
            if self.bits_remaining == 0 {
                self.bits_remaining = 8;
                if let Some(buf) = self.sample_buffer.take() {
                    self.silence = false;
                    self.shift_register = buf;
                    if self.bytes_remaining > 0 {
                        self.dma_needed = true;
                    }
                } else {
                    self.silence = true;
                }
            }
        } else {
            self.timer_value -= 1;
        }
    }

    pub fn check_dma(&mut self) -> Option<u16> {
        if self.dma_needed && self.bytes_remaining > 0 && self.sample_buffer.is_none() {
            let addr = self.current_address;
            self.current_address = if self.current_address == 0xFFFF {
                0x8000
            } else {
                self.current_address + 1
            };

            self.bytes_remaining -= 1;
            if self.bytes_remaining == 0 {
                if self.loop_flag {
                    self.restart();
                } else if self.irq_enabled {
                    self.irq_active = true;
                }
            }
            Some(addr)
        } else {
            None
        }
    }

    pub fn output(&self) -> u8 {
        self.output_level
    }
}
