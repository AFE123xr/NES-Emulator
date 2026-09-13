use super::Mapper;
use crate::cartridge::header::Mirroring;

pub struct Mapper001 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    prg_ram: Vec<u8>,
    is_chr_ram: bool,

    shift_reg: u8,
    shift_count: u8,
    control: u8,
    chr_bank_0: u8,
    chr_bank_1: u8,
    prg_bank: u8,
    last_write_cycle: u64,
}

impl Mapper001 {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>, prg_ram_size: usize) -> Self {
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
            shift_reg: 0,
            shift_count: 0,
            control: 0x0C, // Mode 3: fix $C000 to last bank
            chr_bank_0: 0,
            chr_bank_1: 0,
            prg_bank: 0,
            last_write_cycle: u64::MAX,
        }
    }

    fn prg_banks_16k(&self) -> usize {
        self.prg_rom.len() / 16384
    }

    fn prg_offset(&self, addr: u16) -> usize {
        let bank_mode = (self.control >> 2) & 0x03;
        let num_banks = self.prg_banks_16k();
        let selected_bank = (self.prg_bank & 0x0F) as usize % num_banks;

        match bank_mode {
            0 | 1 => {
                // 32 KB mode
                let bank = (selected_bank & !1) % num_banks;
                bank * 16384 + ((addr - 0x8000) as usize)
            }
            2 => {
                // Fix first bank at $8000, switch 16 KB bank at $C000
                if addr < 0xC000 {
                    (addr - 0x8000) as usize
                } else {
                    selected_bank * 16384 + ((addr - 0xC000) as usize)
                }
            }
            3 => {
                // Fix last bank at $C000, switch 16 KB bank at $8000
                if addr < 0xC000 {
                    selected_bank * 16384 + ((addr - 0x8000) as usize)
                } else {
                    let last_bank = num_banks.saturating_sub(1);
                    last_bank * 16384 + ((addr - 0xC000) as usize)
                }
            }
            _ => unreachable!(),
        }
    }

    fn chr_offset(&self, addr: u16) -> usize {
        let chr_mode = (self.control >> 4) & 1;
        let num_4k_banks = (self.chr.len() / 4096).max(1);

        if chr_mode == 0 {
            // 8 KB mode
            let bank = ((self.chr_bank_0 & !1) as usize) % num_4k_banks;
            bank * 4096 + (addr as usize)
        } else {
            // 4 KB mode
            if addr < 0x1000 {
                let bank = (self.chr_bank_0 as usize) % num_4k_banks;
                bank * 4096 + (addr as usize)
            } else {
                let bank = (self.chr_bank_1 as usize) % num_4k_banks;
                bank * 4096 + ((addr - 0x1000) as usize)
            }
        }
    }
}

impl Mapper for Mapper001 {
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

    fn write_prg(&mut self, addr: u16, data: u8, cycle: u64) {
        match addr {
            0x6000..=0x7FFF => {
                let offset = (addr - 0x6000) as usize;
                if offset < self.prg_ram.len() {
                    self.prg_ram[offset] = data;
                }
            }
            0x8000..=0xFFFF => {
                // Real MMC1 ignores writes that occur on consecutive CPU clock cycles.
                if cycle != 0 && cycle == self.last_write_cycle.wrapping_add(1) {
                    return;
                }
                self.last_write_cycle = cycle;

                if (data & 0x80) != 0 {
                    self.shift_reg = 0;
                    self.shift_count = 0;
                    self.control |= 0x0C;
                } else {
                    self.shift_reg |= (data & 1) << self.shift_count;
                    self.shift_count += 1;

                    if self.shift_count == 5 {
                        let val = self.shift_reg;
                        match (addr >> 13) & 0x03 {
                            0 => self.control = val,
                            1 => self.chr_bank_0 = val,
                            2 => self.chr_bank_1 = val,
                            3 => self.prg_bank = val,
                            _ => unreachable!(),
                        }
                        self.shift_reg = 0;
                        self.shift_count = 0;
                    }
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
        match self.control & 0x03 {
            0 => Mirroring::SingleScreenLower,
            1 => Mirroring::SingleScreenUpper,
            2 => Mirroring::Vertical,
            3 => Mirroring::Horizontal,
            _ => unreachable!(),
        }
    }

    fn prg_ram(&self) -> &[u8] {
        &self.prg_ram
    }

    fn prg_ram_mut(&mut self) -> &mut [u8] {
        &mut self.prg_ram
    }
}
