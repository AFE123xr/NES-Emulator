#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    SingleScreenLower,
    SingleScreenUpper,
    FourScreen,
}

#[derive(Debug, Clone)]
pub struct RomHeader {
    pub prg_rom_size: usize, // In bytes
    pub chr_rom_size: usize, // In bytes (0 means CHR RAM)
    pub prg_ram_size: usize, // In bytes
    pub mapper_id: u8,
    pub mirroring: Mirroring,
    pub has_battery: bool,
    pub has_trainer: bool,
}

impl RomHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 16 {
            return Err("Header too short (less than 16 bytes)".into());
        }

        if &bytes[0..4] != b"NES\x1A" {
            return Err("Invalid NES magic number".into());
        }

        let prg_rom_units = bytes[4] as usize;
        let chr_rom_units = bytes[5] as usize;
        let flags_6 = bytes[6];
        let flags_7 = bytes[7];
        let flags_8 = bytes[8];

        let prg_rom_size = prg_rom_units * 16 * 1024;
        let chr_rom_size = chr_rom_units * 8 * 1024;
        let prg_ram_size = if flags_8 == 0 {
            8 * 1024
        } else {
            flags_8 as usize * 8 * 1024
        };

        let has_trainer = (flags_6 & 0x04) != 0;
        let has_battery = (flags_6 & 0x02) != 0;

        let mirroring = if (flags_6 & 0x08) != 0 {
            Mirroring::FourScreen
        } else if (flags_6 & 0x01) != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let mapper_lower = flags_6 >> 4;
        let mapper_upper = flags_7 & 0xF0;
        let mapper_id = mapper_upper | mapper_lower;

        Ok(Self {
            prg_rom_size,
            chr_rom_size,
            prg_ram_size,
            mapper_id,
            mirroring,
            has_battery,
            has_trainer,
        })
    }
}
