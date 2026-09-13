use crate::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn epoch_to_timestamp(epoch_secs: u64) -> String {
    let secs_per_day = 86400;
    let mut days = (epoch_secs / secs_per_day) as i64;
    let rem_secs = (epoch_secs % secs_per_day) as u32;

    let hour = rem_secs / 3600;
    let min = (rem_secs % 3600) / 60;
    let sec = rem_secs % 60;

    // Convert days since Jan 1 1970 to YYYY-MM-DD
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1024 + doe / 1460 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        year, m, d, hour, min, sec
    )
}

pub fn save_screenshot(
    frame_buffer: &[u8; SCREEN_WIDTH * SCREEN_HEIGHT * 3],
    rom_title: &str,
) -> Result<String, String> {
    let dir = Path::new("screenshots");
    if !dir.exists() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create screenshots directory: {}", e))?;
    }

    let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => epoch_to_timestamp(d.as_secs()),
        Err(_) => "screenshot".to_string(),
    };

    // Sanitize title for filename
    let clean_title: String = rom_title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let filename = format!("screenshots/{}_{}.bmp", clean_title, timestamp);

    let width = SCREEN_WIDTH as u32;
    let height = SCREEN_HEIGHT as u32;
    let row_bytes = width * 3; // 256 * 3 = 768 (already multiple of 4, no padding needed)
    let image_size = row_bytes * height;
    let file_size = 54 + image_size;

    let mut bmp_data = Vec::with_capacity(file_size as usize);

    // 14-byte BITMAPFILEHEADER
    bmp_data.extend_from_slice(b"BM");
    bmp_data.extend_from_slice(&file_size.to_le_bytes());
    bmp_data.extend_from_slice(&0u16.to_le_bytes()); // Reserved 1
    bmp_data.extend_from_slice(&0u16.to_le_bytes()); // Reserved 2
    bmp_data.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel data

    // 40-byte BITMAPINFOHEADER
    bmp_data.extend_from_slice(&40u32.to_le_bytes()); // Header size
    bmp_data.extend_from_slice(&(width as i32).to_le_bytes());
    bmp_data.extend_from_slice(&(height as i32).to_le_bytes()); // Bottom-up bitmap
    bmp_data.extend_from_slice(&1u16.to_le_bytes()); // Color planes
    bmp_data.extend_from_slice(&24u16.to_le_bytes()); // Bits per pixel
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Compression (0 = BI_RGB)
    bmp_data.extend_from_slice(&image_size.to_le_bytes());
    bmp_data.extend_from_slice(&2835i32.to_le_bytes()); // X pixels per meter (~72 DPI)
    bmp_data.extend_from_slice(&2835i32.to_le_bytes()); // Y pixels per meter (~72 DPI)
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Colors in color table
    bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Important colors

    // Pixel data: BMP expects bottom-up rows in BGR format
    for y in (0..SCREEN_HEIGHT).rev() {
        let row_start = y * SCREEN_WIDTH * 3;
        for x in 0..SCREEN_WIDTH {
            let idx = row_start + x * 3;
            let r = frame_buffer[idx];
            let g = frame_buffer[idx + 1];
            let b = frame_buffer[idx + 2];
            // Write in BGR order
            bmp_data.push(b);
            bmp_data.push(g);
            bmp_data.push(r);
        }
    }

    let mut file = File::create(&filename)
        .map_err(|e| format!("Failed to create screenshot file '{}': {}", filename, e))?;
    file.write_all(&bmp_data)
        .map_err(|e| format!("Failed to write screenshot file '{}': {}", filename, e))?;

    Ok(filename)
}
