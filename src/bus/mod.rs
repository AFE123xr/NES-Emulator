use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::cheats::CheatEntry;
use crate::controller::Controller;
use crate::cpu::Memory;
use crate::ppu::Ppu;

pub struct Bus {
    pub cpu_ram: [u8; 2048],
    pub cartridge: Cartridge,
    pub ppu: Ppu,
    pub apu: Apu,
    pub controller1: Controller,
    pub controller2: Controller,

    pub dma_cycles: usize,
    pub cheats: Vec<CheatEntry>,
    pub cheats_enabled: bool,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cpu_ram: [0; 2048],
            cartridge,
            ppu: Ppu::new(),
            apu: Apu::new(),
            controller1: Controller::new(),
            controller2: Controller::new(),
            dma_cycles: 0,
            cheats: Vec::new(),
            cheats_enabled: true,
        }
    }

    pub fn apply_cheats(&mut self) {
        if !self.cheats_enabled || self.cheats.is_empty() {
            return;
        }
        for cheat in &self.cheats {
            if !cheat.enabled {
                continue;
            }
            match cheat.address {
                0x0000..=0x1FFF => {
                    let idx = (cheat.address & 0x07FF) as usize;
                    if cheat.compare.is_none_or(|cmp| self.cpu_ram[idx] == cmp) {
                        self.cpu_ram[idx] = cheat.value;
                    }
                }
                0x6000..=0x7FFF => {
                    let current = self.cartridge.read_prg(cheat.address);
                    if cheat.compare.is_none_or(|cmp| current == cmp) {
                        self.cartridge
                            .write_prg_cycle(cheat.address, cheat.value, 0);
                    }
                }
                _ => {}
            }
        }
    }

    fn apply_cheat_on_read(&mut self, addr: u16, raw_val: u8) -> u8 {
        if !self.cheats_enabled || self.cheats.is_empty() {
            return raw_val;
        }
        let mut val = raw_val;
        for cheat in &self.cheats {
            if !cheat.enabled {
                continue;
            }
            let addr_matches = match addr {
                0x0000..=0x1FFF => {
                    cheat.address <= 0x1FFF && (cheat.address & 0x07FF) == (addr & 0x07FF)
                }
                _ => cheat.address == addr,
            };
            if addr_matches && cheat.compare.is_none_or(|cmp| val == cmp) {
                val = cheat.value;
                if addr <= 0x1FFF {
                    self.cpu_ram[(addr & 0x07FF) as usize] = val;
                }
            }
        }
        val
    }

    pub fn poll_nmi(&mut self) -> bool {
        if self.ppu.nmi_triggered {
            self.ppu.nmi_triggered = false;
            true
        } else {
            false
        }
    }

    pub fn poll_irq(&self) -> bool {
        self.cartridge.irq_state() || self.apu.irq_state()
    }
}

impl Memory for Bus {
    fn mem_read(&mut self, addr: u16) -> u8 {
        let raw = match addr {
            0x0000..=0x1FFF => self.cpu_ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => return self.ppu.read_register(addr, &self.cartridge),
            0x4000..=0x4013 => return 0, // APU write-only registers
            0x4014 => return 0,          // OAMDMA write-only
            0x4015 => return self.apu.read_status(),
            0x4016 => return self.controller1.read(),
            0x4017 => return self.controller2.read(),
            0x4018..=0x401F => return 0, // Disabled
            0x4020..=0xFFFF => self.cartridge.read_prg(addr),
        };
        self.apply_cheat_on_read(addr, raw)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.mem_write_cycle(addr, data, 0);
    }

    fn mem_write_cycle(&mut self, addr: u16, data: u8, cycle: u64) {
        match addr {
            0x0000..=0x1FFF => {
                self.cpu_ram[(addr & 0x07FF) as usize] = data;
                if self.cheats_enabled && !self.cheats.is_empty() {
                    self.apply_cheats();
                }
            }
            0x2000..=0x3FFF => {
                self.ppu.write_register(addr, data, &mut self.cartridge);
            }
            0x4000..=0x4013 | 0x4015 => {
                self.apu.write_register(addr, data);
            }
            0x4014 => {
                // OAMDMA: 256 bytes copied from CPU memory to PPU OAM
                let page_addr = (data as u16) << 8;
                let mut oam_bytes = [0u8; 256];
                for (i, byte) in oam_bytes.iter_mut().enumerate() {
                    *byte = self.mem_read(page_addr + (i as u16));
                }
                self.ppu.write_oam_dma(&oam_bytes);
                self.dma_cycles += 513;
            }
            0x4016 => {
                self.controller1.write_strobe(data);
                self.controller2.write_strobe(data);
            }
            0x4017 => {
                self.apu.write_register(addr, data);
            }
            0x4018..=0x401F => {}
            0x4020..=0xFFFF => {
                self.cartridge.write_prg_cycle(addr, data, cycle);
                if self.cheats_enabled
                    && !self.cheats.is_empty()
                    && (0x6000..=0x7FFF).contains(&addr)
                {
                    self.apply_cheats();
                }
            }
        }
    }
}
