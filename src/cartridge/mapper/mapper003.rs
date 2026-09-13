use super::Mapper;
use crate::cartridge::header::Mirroring;

pub struct Mapper003 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
    is_chr_ram: bool,
    chr_bank: usize,
}

impl Mapper003 {
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
            mirroring,
            is_chr_ram,
            chr_bank: 0,
        }
    }
}

impl Mapper for Mapper003 {
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
                let mut offset = (addr - 0x8000) as usize;
                if self.prg_rom.len() == 16384 {
                    offset %= 16384;
                }
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
                self.chr_bank = (data & 0x03) as usize;
            }
            _ => {}
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        let num_8k_banks = (self.chr.len() / 8192).max(1);
        let bank = self.chr_bank % num_8k_banks;
        let offset = bank * 8192 + (addr as usize);
        if offset < self.chr.len() {
            self.chr[offset]
        } else {
            0
        }
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        if self.is_chr_ram {
            let num_8k_banks = (self.chr.len() / 8192).max(1);
            let bank = self.chr_bank % num_8k_banks;
            let offset = bank * 8192 + (addr as usize);
            if offset < self.chr.len() {
                self.chr[offset] = data;
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
