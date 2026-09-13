use super::Mapper;
use crate::cartridge::header::Mirroring;

pub struct Mapper007 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    prg_ram: Vec<u8>,
    is_chr_ram: bool,
    prg_bank: usize,
    mirroring: Mirroring,
}

impl Mapper007 {
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
            prg_bank: 0,
            mirroring: Mirroring::SingleScreenLower,
        }
    }
}

impl Mapper for Mapper007 {
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
                let num_32k_banks = (self.prg_rom.len() / 32768).max(1);
                let bank = self.prg_bank % num_32k_banks;
                let offset = bank * 32768 + ((addr - 0x8000) as usize);
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
            0x8000..=0xFFFF => {
                self.prg_bank = (data & 0x07) as usize;
                self.mirroring = if (data & 0x10) != 0 {
                    Mirroring::SingleScreenUpper
                } else {
                    Mirroring::SingleScreenLower
                };
            }
            _ => {}
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        let addr = addr as usize;
        if addr < self.chr.len() {
            self.chr[addr]
        } else {
            0
        }
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        if self.is_chr_ram {
            let addr = addr as usize;
            if addr < self.chr.len() {
                self.chr[addr] = data;
            }
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn prg_ram(&self) -> &[u8] {
        &self.prg_ram
    }

    fn prg_ram_mut(&mut self) -> &mut [u8] {
        &mut self.prg_ram
    }
}
