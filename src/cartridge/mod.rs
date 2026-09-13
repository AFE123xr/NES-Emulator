pub mod header;
pub mod mapper;

use header::{Mirroring, RomHeader};
use mapper::{
    mapper000::Mapper000, mapper001::Mapper001, mapper002::Mapper002, mapper003::Mapper003,
    mapper004::Mapper004, mapper007::Mapper007, Mapper,
};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub struct Cartridge {
    pub header: RomHeader,
    pub mapper: Box<dyn Mapper>,
}

impl Cartridge {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let header = RomHeader::parse(bytes)?;

        let mut offset = 16;
        if header.has_trainer {
            offset += 512;
        }

        if bytes.len() < offset + header.prg_rom_size {
            return Err("File too small for PRG ROM size".into());
        }

        let prg_rom = bytes[offset..offset + header.prg_rom_size].to_vec();
        offset += header.prg_rom_size;

        let chr_rom = if header.chr_rom_size > 0 {
            if bytes.len() < offset + header.chr_rom_size {
                return Err("File too small for CHR ROM size".into());
            }
            bytes[offset..offset + header.chr_rom_size].to_vec()
        } else {
            Vec::new()
        };

        let mapper: Box<dyn Mapper> = match header.mapper_id {
            0 => Box::new(Mapper000::new(
                prg_rom,
                chr_rom,
                header.prg_ram_size,
                header.mirroring,
            )),
            1 => Box::new(Mapper001::new(prg_rom, chr_rom, header.prg_ram_size)),
            2 => Box::new(Mapper002::new(
                prg_rom,
                chr_rom,
                header.prg_ram_size,
                header.mirroring,
            )),
            3 => Box::new(Mapper003::new(
                prg_rom,
                chr_rom,
                header.prg_ram_size,
                header.mirroring,
            )),
            4 => Box::new(Mapper004::new(
                prg_rom,
                chr_rom,
                header.prg_ram_size,
                header.mirroring,
            )),
            7 => Box::new(Mapper007::new(prg_rom, chr_rom, header.prg_ram_size)),
            id => return Err(format!("Unsupported mapper ID: {}", id)),
        };

        Ok(Self { header, mapper })
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let mut file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        Self::from_bytes(&bytes)
    }

    pub fn read_prg(&self, addr: u16) -> u8 {
        self.mapper.read_prg(addr)
    }

    pub fn write_prg(&mut self, addr: u16, data: u8) {
        self.mapper.write_prg(addr, data, 0);
    }

    pub fn write_prg_cycle(&mut self, addr: u16, data: u8, cycle: u64) {
        self.mapper.write_prg(addr, data, cycle);
    }

    pub fn read_chr(&self, addr: u16) -> u8 {
        self.mapper.read_chr(addr)
    }

    pub fn write_chr(&mut self, addr: u16, data: u8) {
        self.mapper.write_chr(addr, data);
    }

    pub fn mirroring(&self) -> Mirroring {
        self.mapper.mirroring()
    }

    pub fn irq_state(&self) -> bool {
        self.mapper.irq_state()
    }

    pub fn step_scanline(&mut self) {
        self.mapper.step_scanline();
    }
}
