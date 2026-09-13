use crate::apu::Apu;
use crate::cartridge::Cartridge;
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
        }
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
        match addr {
            0x0000..=0x1FFF => self.cpu_ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.read_register(addr, &self.cartridge),
            0x4000..=0x4013 => 0, // APU write-only registers
            0x4014 => 0,          // OAMDMA write-only
            0x4015 => self.apu.read_status(),
            0x4016 => self.controller1.read(),
            0x4017 => self.controller2.read(),
            0x4018..=0x401F => 0, // Disabled
            0x4020..=0xFFFF => self.cartridge.read_prg(addr),
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.mem_write_cycle(addr, data, 0);
    }

    fn mem_write_cycle(&mut self, addr: u16, data: u8, cycle: u64) {
        match addr {
            0x0000..=0x1FFF => {
                self.cpu_ram[(addr & 0x07FF) as usize] = data;
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
            }
        }
    }
}
