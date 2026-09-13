use nes::cartridge::header::Mirroring;
use nes::cartridge::Cartridge;
use nes::ppu::registers::PpuStatus;
use nes::ppu::Ppu;

fn create_test_cartridge(mirroring: Mirroring) -> Cartridge {
    let mut rom_bytes = vec![0u8; 16 + 16384 + 8192];
    rom_bytes[0..4].copy_from_slice(b"NES\x1A");
    rom_bytes[4] = 1; // 16KB PRG
    rom_bytes[5] = 1; // 8KB CHR
    rom_bytes[6] = match mirroring {
        Mirroring::Vertical => 0x01,
        Mirroring::Horizontal => 0x00,
        Mirroring::FourScreen => 0x08,
        _ => 0x00,
    };
    Cartridge::from_bytes(&rom_bytes).unwrap()
}

#[test]
fn test_ppu_vram_mirroring_vertical() {
    let mut cart = create_test_cartridge(Mirroring::Vertical);
    let mut ppu = Ppu::new();

    // Nametable 0 ($2000) and Nametable 2 ($2800) should be the same
    ppu.write_vram(0x2000, 0x42, &mut cart);
    assert_eq!(ppu.read_vram(0x2800, &cart), 0x42);

    // Nametable 1 ($2400) and Nametable 3 ($2C00) should be the same
    ppu.write_vram(0x2400, 0x99, &mut cart);
    assert_eq!(ppu.read_vram(0x2C00, &cart), 0x99);

    // Nametable 0 and Nametable 1 should be distinct
    assert_ne!(ppu.read_vram(0x2000, &cart), ppu.read_vram(0x2400, &cart));
}

#[test]
fn test_ppu_vram_mirroring_horizontal() {
    let mut cart = create_test_cartridge(Mirroring::Horizontal);
    let mut ppu = Ppu::new();

    // Nametable 0 ($2000) and Nametable 1 ($2400) should be the same
    ppu.write_vram(0x2000, 0x55, &mut cart);
    assert_eq!(ppu.read_vram(0x2400, &cart), 0x55);

    // Nametable 2 ($2800) and Nametable 3 ($2C00) should be the same
    ppu.write_vram(0x2800, 0xAA, &mut cart);
    assert_eq!(ppu.read_vram(0x2C00, &cart), 0xAA);

    // Nametable 0 and Nametable 2 should be distinct
    assert_ne!(ppu.read_vram(0x2000, &cart), ppu.read_vram(0x2800, &cart));
}

#[test]
fn test_ppu_palette_ram_mirroring() {
    let mut cart = create_test_cartridge(Mirroring::Horizontal);
    let mut ppu = Ppu::new();

    // Write to $3F00 (Universal background color)
    ppu.write_vram(0x3F00, 0x0F, &mut cart);
    // Should be mirrored at $3F10, $3F20, etc.
    assert_eq!(ppu.read_vram(0x3F10, &cart), 0x0F);
    assert_eq!(ppu.read_vram(0x3F20, &cart), 0x0F);

    // Mirrors for $3F04, $3F08, $3F0C at $3F14, $3F18, $3F1C
    ppu.write_vram(0x3F04, 0x1A, &mut cart);
    assert_eq!(ppu.read_vram(0x3F14, &cart), 0x1A);

    ppu.write_vram(0x3F08, 0x2B, &mut cart);
    assert_eq!(ppu.read_vram(0x3F18, &cart), 0x2B);

    ppu.write_vram(0x3F0C, 0x3C, &mut cart);
    assert_eq!(ppu.read_vram(0x3F1C, &cart), 0x3C);
}

#[test]
fn test_ppu_status_clears_vblank_and_w() {
    let cart = create_test_cartridge(Mirroring::Horizontal);
    let mut ppu = Ppu::new();

    // Manually set vblank and w
    ppu.status |= PpuStatus::VBLANK_STARTED;
    ppu.w = true;

    let val = ppu.read_register(0x2002, &cart);
    assert_ne!(val & PpuStatus::VBLANK_STARTED, 0);

    // Reading PPUSTATUS clears VBlank flag and resets w
    assert_eq!(ppu.status & PpuStatus::VBLANK_STARTED, 0);
    assert!(!ppu.w);
}

#[test]
fn test_ppu_scroll_and_addr_registers() {
    let mut cart = create_test_cartridge(Mirroring::Horizontal);
    let mut ppu = Ppu::new();

    // Scroll writes:
    // Write 1: X scroll = 0x7D (fine X = 5, coarse X = 15)
    ppu.write_register(0x2005, 0x7D, &mut cart);
    assert_eq!(ppu.fine_x, 5);
    assert_eq!(ppu.t & 0x001F, 15);
    assert!(ppu.w);

    // Write 2: Y scroll = 0x5E (fine Y = 6, coarse Y = 11)
    ppu.write_register(0x2005, 0x5E, &mut cart);
    assert_eq!((ppu.t >> 12) & 0x07, 6);
    assert_eq!((ppu.t >> 5) & 0x1F, 11);
    assert!(!ppu.w);

    // Address writes:
    // Write 1: High byte = 0x21
    ppu.write_register(0x2006, 0x21, &mut cart);
    assert!(ppu.w);
    // Write 2: Low byte = 0x08
    ppu.write_register(0x2006, 0x08, &mut cart);
    assert!(!ppu.w);
    assert_eq!(ppu.v, 0x2108);
}
