use nes::cartridge::Cartridge;
use nes::nes::Nes;

#[test]
fn test_zelda_sprite0_and_scroll() {
    let rom_path = match std::env::var("ROM_PATH").or_else(|_| std::env::var("NES_ROM")) {
        Ok(path) => path,
        Err(_) => {
            eprintln!("ROM_PATH or NES_ROM environment variable not set, skipping test");
            return;
        }
    };
    if !std::path::Path::new(&rom_path).exists() {
        eprintln!("ROM not found at {}, skipping test", rom_path);
        return;
    }
    let cartridge = Cartridge::from_file(&rom_path).expect("Failed to load ROM");
    let mut nes = Nes::new(cartridge);

    // Step 200 frames to title screen
    for _ in 0..200 {
        nes.step_frame();
    }

    println!("Sprite 0 in Title Screen:");
    println!("  Y: {}", nes.bus.ppu.oam[0]);
    println!("  Tile: 0x{:02X}", nes.bus.ppu.oam[1]);
    println!("  Attr: 0x{:02X}", nes.bus.ppu.oam[2]);
    println!("  X: {}", nes.bus.ppu.oam[3]);
}
