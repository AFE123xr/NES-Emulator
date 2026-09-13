use nes::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use nes::save_screenshot;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

#[test]
fn test_screenshot_generation_and_bmp_format() {
    // Generate a test framebuffer with red/green/blue pattern
    let mut frame_buffer = Box::new([0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 3]);
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let idx = (y * SCREEN_WIDTH + x) * 3;
            frame_buffer[idx] = (x % 256) as u8; // R
            frame_buffer[idx + 1] = (y % 240) as u8; // G
            frame_buffer[idx + 2] = 128; // B
        }
    }

    let saved_path_str =
        save_screenshot(&frame_buffer, "test_game").expect("Failed to save screenshot");

    let path = Path::new(&saved_path_str);
    assert!(path.exists(), "Screenshot file should exist");

    // Read and verify BMP header
    let mut file = File::open(path).expect("Failed to open saved screenshot");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("Failed to read screenshot bytes");

    // Total file size: 54 byte header + 256 * 240 * 3 = 184374 bytes
    assert_eq!(bytes.len(), 54 + SCREEN_WIDTH * SCREEN_HEIGHT * 3);

    // Magic number 'BM'
    assert_eq!(&bytes[0..2], b"BM");

    // File size in header
    let file_size = u32::from_le_bytes(bytes[2..6].try_into().unwrap());
    assert_eq!(file_size as usize, bytes.len());

    // Pixel offset = 54
    let offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
    assert_eq!(offset, 54);

    // DIB header size = 40
    let dib_size = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
    assert_eq!(dib_size, 40);

    // Width = 256
    let width = i32::from_le_bytes(bytes[18..22].try_into().unwrap());
    assert_eq!(width, 256);

    // Height = 240
    let height = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
    assert_eq!(height, 240);

    // Color planes = 1
    let planes = u16::from_le_bytes(bytes[26..28].try_into().unwrap());
    assert_eq!(planes, 1);

    // Bits per pixel = 24
    let bpp = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
    assert_eq!(bpp, 24);

    // Compression = 0 (uncompressed RGB)
    let compression = u32::from_le_bytes(bytes[30..34].try_into().unwrap());
    assert_eq!(compression, 0);

    // Clean up test file
    let _ = fs::remove_file(path);
}
