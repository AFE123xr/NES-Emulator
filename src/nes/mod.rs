use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::controller::JoypadButton;
use crate::cpu::{Cpu, Memory};
use crate::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};

pub struct Nes {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Nes {
    pub fn new(cartridge: Cartridge) -> Self {
        let mut bus = Bus::new(cartridge);
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);

        Self { cpu, bus }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.bus.ppu.reset();
        self.bus.apu.reset();
        self.bus.controller1.reset();
        self.bus.controller2.reset();
        self.bus.dma_cycles = 0;
    }

    pub fn set_button_p1(&mut self, button: JoypadButton, pressed: bool) {
        self.bus.controller1.set_button_state(button, pressed);
    }

    pub fn set_button_p2(&mut self, button: JoypadButton, pressed: bool) {
        self.bus.controller2.set_button_state(button, pressed);
    }

    pub fn set_buttons_p1(&mut self, buttons: u8) {
        self.bus.controller1.set_all_buttons(buttons);
    }

    pub fn set_buttons_p2(&mut self, buttons: u8) {
        self.bus.controller2.set_all_buttons(buttons);
    }

    pub fn buttons_p1(&self) -> u8 {
        self.bus.controller1.button_states()
    }

    pub fn buttons_p2(&self) -> u8 {
        self.bus.controller2.button_states()
    }

    pub fn rom_hash(&self) -> u32 {
        self.bus.cartridge.rom_hash
    }

    /// Computes a fast deterministic 32-bit FNV-1a checksum of core emulator state
    /// (CPU registers, CPU RAM, PPU registers/VRAM/OAM/palette) to verify lockstep sync.
    pub fn state_checksum(&self) -> u32 {
        let mut hash: u32 = 0x811c_9dc5;
        let mut feed = |bytes: &[u8]| {
            for &b in bytes {
                hash ^= b as u32;
                hash = hash.wrapping_mul(0x0100_0193);
            }
        };

        feed(&[
            self.cpu.registers.a,
            self.cpu.registers.x,
            self.cpu.registers.y,
            self.cpu.registers.sp,
            self.cpu.registers.p,
        ]);
        feed(&self.cpu.registers.pc.to_le_bytes());
        feed(&self.bus.cpu_ram);
        feed(&self.bus.ppu.vram);
        feed(&self.bus.ppu.palette_ram);
        feed(&self.bus.ppu.oam);
        feed(&[
            self.bus.ppu.ctrl,
            self.bus.ppu.mask,
            self.bus.ppu.status,
            self.bus.ppu.fine_x,
            self.bus.ppu.w as u8,
        ]);
        feed(&self.bus.ppu.v.to_le_bytes());
        feed(&self.bus.ppu.t.to_le_bytes());
        hash
    }

    pub fn step(&mut self) -> u8 {
        // DMC DMA check
        if let Some(dmc_addr) = self.bus.apu.dmc.check_dma() {
            let sample = self.bus.mem_read(dmc_addr);
            self.bus.apu.dmc.load_sample_buffer(sample);
            // DMC DMA steals up to 4 CPU cycles
            for _ in 0..12 {
                self.bus.ppu.step(&mut self.bus.cartridge);
            }
            for _ in 0..4 {
                self.bus.apu.step();
            }
        }

        // DMA cycle handling from OAMDMA
        if self.bus.dma_cycles > 0 {
            let cycles = self.bus.dma_cycles;
            self.bus.dma_cycles = 0;
            for _ in 0..(cycles * 3) {
                self.bus.ppu.step(&mut self.bus.cartridge);
            }
            for _ in 0..cycles {
                self.bus.apu.step();
            }
        }

        // Check interrupts before executing instruction
        if self.bus.poll_nmi() {
            self.cpu.trigger_nmi();
        }
        self.cpu.set_irq(self.bus.poll_irq());

        // Step CPU
        let cpu_cycles = self.cpu.step(&mut self.bus);

        // Step PPU (3 PPU cycles per 1 CPU cycle)
        for _ in 0..(cpu_cycles * 3) {
            self.bus.ppu.step(&mut self.bus.cartridge);
        }

        // Step APU (1 APU step per CPU cycle)
        for _ in 0..cpu_cycles {
            self.bus.apu.step();
        }

        cpu_cycles
    }

    pub fn step_frame(&mut self) -> &[u8; SCREEN_WIDTH * SCREEN_HEIGHT * 3] {
        while !self.bus.ppu.frame_complete {
            self.step();
        }
        self.bus.ppu.frame_complete = false;
        self.bus.apu.flush_samples();
        &self.bus.ppu.frame_buffer
    }
}
