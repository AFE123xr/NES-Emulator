#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoypadButton {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
}

pub struct Controller {
    button_states: u8,
    shift_index: u8,
    strobe: bool,
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

impl Controller {
    pub fn new() -> Self {
        Self {
            button_states: 0,
            shift_index: 0,
            strobe: false,
        }
    }

    pub fn set_button_state(&mut self, button: JoypadButton, pressed: bool) {
        let bit = 1 << (button as u8);
        if pressed {
            self.button_states |= bit;
        } else {
            self.button_states &= !bit;
        }
    }

    pub fn write_strobe(&mut self, val: u8) {
        self.strobe = (val & 1) != 0;
        if self.strobe {
            self.shift_index = 0;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.shift_index > 7 {
            return 1;
        }

        let bit = (self.button_states >> self.shift_index) & 1;
        if !self.strobe {
            self.shift_index += 1;
        }
        bit
    }
}
