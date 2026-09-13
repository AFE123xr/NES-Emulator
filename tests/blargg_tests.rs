use nes::bus::Bus;
use nes::cartridge::Cartridge;
use nes::cpu::{Cpu, Memory};

#[test]
fn test_blargg_official_only() {
    let rom_path = std::env::var("BLARGG_ROM")
        .unwrap_or_else(|_| "tests/fixtures/official_only.nes".to_string());
    let rom_bytes = std::fs::read(&rom_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read official_only fixture from {}: {}",
            rom_path, e
        )
    });
    let cartridge =
        Cartridge::from_bytes(&rom_bytes).expect("Failed to parse official_only cartridge");

    let mut bus = Bus::new(cartridge);
    let mut cpu = Cpu::new();
    cpu.reset(&mut bus);

    // Run until status at $6000 is 0 (pass) or non-0 (fail after 0x80)
    let max_cycles: u64 = 50_000_000;
    let mut passed = false;

    while cpu.total_cycles < max_cycles {
        if bus.poll_nmi() {
            cpu.trigger_nmi();
        }
        cpu.set_irq(bus.poll_irq());

        let cycles = cpu.step(&mut bus);
        for _ in 0..(cycles * 3) {
            bus.ppu.step(&mut bus.cartridge);
        }
        for _ in 0..cycles {
            bus.apu.step();
        }

        let b4 = bus.mem_read(0x6004);
        if b4 != 0 {
            let mut output = String::new();
            let mut addr = 0x6004;
            loop {
                let b = bus.mem_read(addr);
                if b == 0 || output.len() > 1000 {
                    break;
                }
                output.push(b as char);
                addr += 1;
            }

            if output.contains("Passed") {
                println!("Blargg test output:\n{}", output.trim());
                passed = true;
                break;
            }
        }
    }

    assert!(passed, "Blargg test did not report Passed");
}
