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
    }

    pub fn set_button_p1(&mut self, button: JoypadButton, pressed: bool) {
        self.bus.controller1.set_button_state(button, pressed);
    }

    pub fn set_button_p2(&mut self, button: JoypadButton, pressed: bool) {
        self.bus.controller2.set_button_state(button, pressed);
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
