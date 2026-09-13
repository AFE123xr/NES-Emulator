use super::Mapper;
use crate::cartridge::header::Mirroring;

pub struct Mapper004 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    prg_ram: Vec<u8>,
    is_chr_ram: bool,

    bank_select: u8,
    bank_registers: [u8; 8],
    mirroring: Mirroring,

    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_active: bool,
}

impl Mapper004 {
    pub fn new(
        prg_rom: Vec<u8>,
        chr_rom: Vec<u8>,
        prg_ram_size: usize,
        mirroring: Mirroring,
    ) -> Self {
        let (chr, is_chr_ram) = if chr_rom.is_empty() {
            (vec![0; 8192], true)
        } else {
            (chr_rom, false)
        };

        let ram_size = if prg_ram_size == 0 {
            8192
        } else {
            prg_ram_size
        };

        Self {
            prg_rom,
            chr,
            prg_ram: vec![0; ram_size],
            is_chr_ram,
            bank_select: 0,
            bank_registers: [0; 8],
            mirroring,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_active: false,
        }
    }

    fn prg_banks_8k(&self) -> usize {
        (self.prg_rom.len() / 8192).max(1)
    }

    fn prg_offset(&self, addr: u16) -> usize {
        let num_banks = self.prg_banks_8k();
        let prg_mode = (self.bank_select & 0x40) != 0;

        let bank = match (addr, prg_mode) {
            (0x8000..=0x9FFF, false) => (self.bank_registers[6] as usize) % num_banks,
            (0x8000..=0x9FFF, true) => num_banks.saturating_sub(2),

            (0xA000..=0xBFFF, _) => (self.bank_registers[7] as usize) % num_banks,

            (0xC000..=0xDFFF, false) => num_banks.saturating_sub(2),
            (0xC000..=0xDFFF, true) => (self.bank_registers[6] as usize) % num_banks,

            (0xE000..=0xFFFF, _) => num_banks.saturating_sub(1),
            _ => 0,
        };

        let offset_in_bank = (addr & 0x1FFF) as usize;
        bank * 8192 + offset_in_bank
    }

    fn chr_offset(&self, addr: u16) -> usize {
        let num_1k_banks = (self.chr.len() / 1024).max(1);
        let chr_inversion = (self.bank_select & 0x80) != 0;

        let (bank, offset) = match (addr, chr_inversion) {
            // Standard mapping
            (0x0000..=0x07FF, false) => ((self.bank_registers[0] & 0xFE) as usize, addr as usize),
            (0x0800..=0x0FFF, false) => (
                ((self.bank_registers[1] & 0xFE) as usize),
                (addr - 0x0800) as usize,
            ),
            (0x1000..=0x13FF, false) => (self.bank_registers[2] as usize, (addr - 0x1000) as usize),
            (0x1400..=0x17FF, false) => (self.bank_registers[3] as usize, (addr - 0x1400) as usize),
            (0x1800..=0x1BFF, false) => (self.bank_registers[4] as usize, (addr - 0x1800) as usize),
            (0x1C00..=0x1FFF, false) => (self.bank_registers[5] as usize, (addr - 0x1C00) as usize),

            // Inverted mapping
            (0x0000..=0x03FF, true) => (self.bank_registers[2] as usize, addr as usize),
            (0x0400..=0x07FF, true) => (self.bank_registers[3] as usize, (addr - 0x0400) as usize),
            (0x0800..=0x0BFF, true) => (self.bank_registers[4] as usize, (addr - 0x0800) as usize),
            (0x0C00..=0x0FFF, true) => (self.bank_registers[5] as usize, (addr - 0x0C00) as usize),
            (0x1000..=0x17FF, true) => (
                (self.bank_registers[0] & 0xFE) as usize,
                (addr - 0x1000) as usize,
            ),
            (0x1800..=0x1FFF, true) => (
                ((self.bank_registers[1] & 0xFE) as usize),
                (addr - 0x1800) as usize,
            ),
            _ => (0, 0),
        };

        (bank % num_1k_banks) * 1024 + offset
    }
}

impl Mapper for Mapper004 {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let offset = (addr - 0x6000) as usize;
                if offset < self.prg_ram.len() {
                    self.prg_ram[offset]
                } else {
                    0
                }
            }
            0x8000..=0xFFFF => {
                let offset = self.prg_offset(addr);
                if offset < self.prg_rom.len() {
                    self.prg_rom[offset]
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn write_prg(&mut self, addr: u16, data: u8, _cycle: u64) {
        match addr {
            0x6000..=0x7FFF => {
                let offset = (addr - 0x6000) as usize;
                if offset < self.prg_ram.len() {
                    self.prg_ram[offset] = data;
                }
            }
            0x8000..=0x9FFF => {
                if (addr & 1) == 0 {
                    self.bank_select = data;
                } else {
                    let reg = (self.bank_select & 0x07) as usize;
                    self.bank_registers[reg] = data;
                }
            }
            0xA000..=0xBFFF => {
                if (addr & 1) == 0 {
                    self.mirroring = if (data & 1) == 0 {
                        Mirroring::Vertical
                    } else {
                        Mirroring::Horizontal
                    };
                } else {
                    // PRG RAM protect (ignored for now)
                }
            }
            0xC000..=0xDFFF => {
                if (addr & 1) == 0 {
                    self.irq_latch = data;
                } else {
                    self.irq_reload = true;
                }
            }
            0xE000..=0xFFFF => {
                if (addr & 1) == 0 {
                    self.irq_enabled = false;
                    self.irq_active = false;
                } else {
                    self.irq_enabled = true;
                }
            }
            _ => {}
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        let offset = self.chr_offset(addr);
        if offset < self.chr.len() {
            self.chr[offset]
        } else {
            0
        }
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        if self.is_chr_ram {
            let offset = self.chr_offset(addr);
            if offset < self.chr.len() {
                self.chr[offset] = data;
            }
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn irq_state(&self) -> bool {
        self.irq_active
    }

    fn step_scanline(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter = self.irq_counter.saturating_sub(1);
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_active = true;
        }
    }

    fn prg_ram(&self) -> &[u8] {
        &self.prg_ram
    }

    fn prg_ram_mut(&mut self) -> &mut [u8] {
        &mut self.prg_ram
    }
}
