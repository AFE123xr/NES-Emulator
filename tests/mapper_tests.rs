use nes::cartridge::Cartridge;

fn make_cart(mapper: u8, prg_kb: usize, chr_kb: usize) -> Cartridge {
    let prg_size = prg_kb * 1024;
    let chr_size = chr_kb * 1024;
    let mut bytes = vec![0u8; 16 + prg_size + chr_size];
    bytes[0..4].copy_from_slice(b"NES\x1A");
    bytes[4] = (prg_kb / 16) as u8;
    bytes[5] = (chr_kb / 8) as u8;
    bytes[6] = (mapper & 0x0F) << 4;
    bytes[7] = mapper & 0xF0;

    // Fill PRG with distinctive pattern
    for i in 0..prg_size {
        bytes[16 + i] = ((i / 16384) + 1) as u8;
    }

    Cartridge::from_bytes(&bytes).unwrap()
}

#[test]
fn test_mapper0_nrom16() {
    let cart = make_cart(0, 16, 8);
    // NROM-128: 16KB PRG should be mirrored at $8000 and $C000
    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xC000), 1);
}

#[test]
fn test_mapper0_nrom32() {
    let cart = make_cart(0, 32, 8);
    // NROM-256: 32KB PRG, $8000 is bank 1, $C000 is bank 2
    assert_eq!(cart.read_prg(0x8000), 1);
    assert_eq!(cart.read_prg(0xC000), 2);
}

#[test]
fn test_mapper2_uxrom() {
    let mut cart = make_cart(2, 64, 8); // 4 banks of 16KB
                                        // Fixed last bank at $C000
    assert_eq!(cart.read_prg(0xC000), 4);

    // Initial $8000 bank 0 (value 1)
    assert_eq!(cart.read_prg(0x8000), 1);

    // Switch bank at $8000 to bank 2 (value 3)
    cart.write_prg(0x8000, 2);
    assert_eq!(cart.read_prg(0x8000), 3);
}

#[test]
fn test_mapper4_mmc3_irq() {
    let mut cart = make_cart(4, 128, 64);

    // Set IRQ latch to 2
    cart.write_prg(0xC000, 2);
    // Reload IRQ counter
    cart.write_prg(0xC001, 0);
    // Enable IRQ
    cart.write_prg(0xE001, 0);

    assert!(!cart.irq_state());

    // Scanline 1: counter reloads to 2
    cart.step_scanline();
    assert!(!cart.irq_state());

    // Scanline 2: counter decrements to 1
    cart.step_scanline();
    assert!(!cart.irq_state());

    // Scanline 3: counter decrements to 0 -> triggers IRQ!
    cart.step_scanline();
    assert!(cart.irq_state());

    // Disable IRQ at $E000 clears IRQ
    cart.write_prg(0xE000, 0);
    assert!(!cart.irq_state());
}
