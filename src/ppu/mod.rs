pub mod palette;
pub mod registers;

use crate::cartridge::header::Mirroring;
use crate::cartridge::Cartridge;
use palette::get_color_rgb;
use registers::{PpuCtrl, PpuMask, PpuStatus};

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;

pub struct Ppu {
    // Memory
    pub vram: [u8; 2048],
    pub palette_ram: [u8; 32],
    pub oam: [u8; 256],

    // Registers
    pub ctrl: u8,
    pub mask: u8,
    pub status: u8,
    pub oam_addr: u8,

    // Internal Loopy scrolling registers
    pub v: u16,     // Current VRAM address (15 bits)
    pub t: u16,     // Temporary VRAM address (15 bits)
    pub fine_x: u8, // Fine X scroll (3 bits)
    pub w: bool,    // Write toggle

    // Buffers & Timing
    pub internal_read_buffer: u8,
    pub scanline: u16, // 0 - 261
    pub cycle: u16,    // 0 - 340
    pub frame: u64,
    pub nmi_triggered: bool,
    pub frame_complete: bool,

    // Framebuffer: 256 * 240 * 3 RGB
    pub frame_buffer: Box<[u8; SCREEN_WIDTH * SCREEN_HEIGHT * 3]>,

    // Intermediate scanline buffers for sprite 0 hit and priority
    scanline_bg_color_indices: [u8; SCREEN_WIDTH],
    scanline_bg_palettes: [u8; SCREEN_WIDTH],

    // Latched scroll at start of scanline for background rendering
    pub scanline_start_v: u16,
    pub scanline_start_fine_x: u8,
    pub sprite_0_hit_cycle: Option<u16>,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 2048],
            palette_ram: [0; 32],
            oam: [0; 256],
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            v: 0,
            t: 0,
            fine_x: 0,
            w: false,
            internal_read_buffer: 0,
            scanline: 261, // Pre-render scanline initially
            cycle: 0,
            frame: 0,
            nmi_triggered: false,
            frame_complete: false,
            frame_buffer: Box::new([0; SCREEN_WIDTH * SCREEN_HEIGHT * 3]),
            scanline_bg_color_indices: [0; SCREEN_WIDTH],
            scanline_bg_palettes: [0; SCREEN_WIDTH],
            scanline_start_v: 0,
            scanline_start_fine_x: 0,
            sprite_0_hit_cycle: None,
        }
    }

    pub fn reset(&mut self) {
        self.ctrl = 0;
        self.mask = 0;
        self.status = 0;
        self.oam_addr = 0;
        self.v = 0;
        self.t = 0;
        self.fine_x = 0;
        self.w = false;
        self.internal_read_buffer = 0;
        self.scanline = 261;
        self.cycle = 0;
        self.frame = 0;
        self.nmi_triggered = false;
        self.frame_complete = false;
        self.scanline_start_v = 0;
        self.scanline_start_fine_x = 0;
        self.sprite_0_hit_cycle = None;
    }

    pub fn is_rendering_enabled(&self) -> bool {
        (self.mask & (PpuMask::SHOW_BG | PpuMask::SHOW_SPRITES)) != 0
    }

    fn mirror_vram_addr(&self, addr: u16, mirroring: Mirroring) -> usize {
        let norm = (addr - 0x2000) & 0x0FFF;
        match mirroring {
            Mirroring::Vertical => (norm % 0x0800) as usize,
            Mirroring::Horizontal => {
                if norm < 0x0800 {
                    (norm & 0x03FF) as usize
                } else {
                    (0x0400 + (norm & 0x03FF)) as usize
                }
            }
            Mirroring::SingleScreenLower => (norm & 0x03FF) as usize,
            Mirroring::SingleScreenUpper => (0x0400 + (norm & 0x03FF)) as usize,
            Mirroring::FourScreen => (norm & 0x07FF) as usize,
        }
    }

    fn palette_addr_mirror(addr: u16) -> usize {
        let idx = (addr & 0x1F) as usize;
        match idx {
            0x10 => 0x00,
            0x14 => 0x04,
            0x18 => 0x08,
            0x1C => 0x0C,
            _ => idx,
        }
    }

    pub fn read_register(&mut self, addr: u16, cart: &Cartridge) -> u8 {
        match addr & 0x2007 {
            0x2000 => 0, // Write-only
            0x2001 => 0, // Write-only
            0x2002 => {
                if let Some(hit_cycle) = self.sprite_0_hit_cycle {
                    if self.cycle >= hit_cycle {
                        self.status |= PpuStatus::SPRITE_0_HIT;
                        self.sprite_0_hit_cycle = None;
                    }
                }
                let res = self.status;
                // Clear VBlank bit on read
                self.status &= !PpuStatus::VBLANK_STARTED;
                // Reset address latch write toggle
                self.w = false;
                res
            }
            0x2003 => 0, // Write-only
            0x2004 => self.oam[self.oam_addr as usize],
            0x2005 => 0, // Write-only
            0x2006 => 0, // Write-only
            0x2007 => {
                let vram_addr = self.v & 0x3FFF;
                let increment = if (self.ctrl & PpuCtrl::VRAM_INCREMENT) != 0 {
                    32
                } else {
                    1
                };
                self.v = self.v.wrapping_add(increment) & 0x7FFF;

                if vram_addr < 0x3F00 {
                    let prev_buf = self.internal_read_buffer;
                    self.internal_read_buffer = self.read_vram(vram_addr, cart);
                    prev_buf
                } else {
                    self.internal_read_buffer = self.read_vram(vram_addr - 0x1000, cart);
                    let pal_idx = Self::palette_addr_mirror(vram_addr);
                    self.palette_ram[pal_idx]
                }
            }
            _ => 0,
        }
    }

    pub fn write_register(&mut self, addr: u16, data: u8, cart: &mut Cartridge) {
        match addr & 0x2007 {
            0x2000 => {
                let prev_nmi = (self.ctrl & PpuCtrl::GENERATE_NMI) != 0;
                self.ctrl = data;
                // t: ...NN........ = d: ......NN
                self.t = (self.t & !0x0C00) | (((data as u16) & 0x03) << 10);
                let current_nmi = (self.ctrl & PpuCtrl::GENERATE_NMI) != 0;
                let in_vblank = (self.status & PpuStatus::VBLANK_STARTED) != 0;
                if !prev_nmi && current_nmi && in_vblank {
                    self.nmi_triggered = true;
                }
            }
            0x2001 => {
                self.mask = data;
            }
            0x2002 => {} // Read-only
            0x2003 => {
                self.oam_addr = data;
            }
            0x2004 => {
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                if !self.w {
                    // First write: X scroll
                    // t: ........ ...XXXXX = d >> 3
                    self.t = (self.t & !0x001F) | ((data as u16) >> 3);
                    self.fine_x = data & 0x07;
                    self.w = true;
                } else {
                    // Second write: Y scroll
                    // t: .yyy.. YYYYY..... = (d & 0x07) << 12 | (d >> 3) << 5
                    let fine_y = (data as u16 & 0x07) << 12;
                    let coarse_y = ((data as u16) >> 3) << 5;
                    self.t = (self.t & !0x73E0) | fine_y | coarse_y;
                    self.w = false;
                }
            }
            0x2006 => {
                if !self.w {
                    // First write: high byte
                    // t: ..00YYYY YYYYYYY = (d & 0x3F) << 8
                    self.t = (self.t & 0x00FF) | (((data as u16) & 0x3F) << 8);
                    self.w = true;
                } else {
                    // Second write: low byte
                    self.t = (self.t & 0xFF00) | (data as u16);
                    self.v = self.t;
                    self.w = false;
                }
            }
            0x2007 => {
                let vram_addr = self.v & 0x3FFF;
                self.write_vram(vram_addr, data, cart);
                let increment = if (self.ctrl & PpuCtrl::VRAM_INCREMENT) != 0 {
                    32
                } else {
                    1
                };
                self.v = self.v.wrapping_add(increment) & 0x7FFF;
            }
            _ => {}
        }
    }

    pub fn read_vram(&self, addr: u16, cart: &Cartridge) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.read_chr(addr),
            0x2000..=0x3EFF => {
                let idx = self.mirror_vram_addr(addr, cart.mirroring());
                self.vram[idx]
            }
            0x3F00..=0x3FFF => {
                let idx = Self::palette_addr_mirror(addr);
                self.palette_ram[idx]
            }
            _ => 0,
        }
    }

    pub fn write_vram(&mut self, addr: u16, data: u8, cart: &mut Cartridge) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => cart.write_chr(addr, data),
            0x2000..=0x3EFF => {
                let idx = self.mirror_vram_addr(addr, cart.mirroring());
                self.vram[idx] = data;
            }
            0x3F00..=0x3FFF => {
                let idx = Self::palette_addr_mirror(addr);
                self.palette_ram[idx] = data;
            }
            _ => {}
        }
    }

    pub fn write_oam_dma(&mut self, data: &[u8; 256]) {
        for &byte in data.iter() {
            self.oam[self.oam_addr as usize] = byte;
            self.oam_addr = self.oam_addr.wrapping_add(1);
        }
    }

    fn increment_scroll_x(&mut self) {
        if !self.is_rendering_enabled() {
            return;
        }

        if (self.v & 0x001F) == 31 {
            self.v &= !0x001F;
            self.v ^= 0x0400;
        } else {
            self.v += 1;
        }
    }

    fn increment_scroll_y(&mut self) {
        if !self.is_rendering_enabled() {
            return;
        }

        if (self.v & 0x7000) != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            let mut y = (self.v & 0x03E0) >> 5;
            if y == 29 {
                y = 0;
                self.v ^= 0x0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.v = (self.v & !0x03E0) | (y << 5);
        }
    }

    fn copy_scroll_x(&mut self) {
        if !self.is_rendering_enabled() {
            return;
        }
        // v: ....F.. ...EEEEE = t: ....F.. ...EEEEE
        self.v = (self.v & !0x041F) | (self.t & 0x041F);
    }

    fn copy_scroll_y(&mut self) {
        if !self.is_rendering_enabled() {
            return;
        }
        // v: .IHGF.ED CBA..... = t: .IHGF.ED CBA.....
        self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
    }

    pub fn step(&mut self, cart: &mut Cartridge) {
        // Pre-render scanline (261)
        if self.scanline == 261 {
            if self.cycle == 1 {
                self.status &= !(PpuStatus::VBLANK_STARTED
                    | PpuStatus::SPRITE_0_HIT
                    | PpuStatus::SPRITE_OVERFLOW);
                self.nmi_triggered = false;
                self.sprite_0_hit_cycle = None;
            } else if self.cycle >= 280 && self.cycle <= 304 {
                self.copy_scroll_y();
            }
        }

        // Visible scanlines (0..=239)
        if self.scanline < 240 {
            if self.cycle == 0 {
                self.scanline_start_v = self.v;
                self.scanline_start_fine_x = self.fine_x;
                self.sprite_0_hit_cycle = None;
                self.render_scanline(cart);
            }

            if let Some(hit_cycle) = self.sprite_0_hit_cycle {
                if self.cycle >= hit_cycle {
                    self.status |= PpuStatus::SPRITE_0_HIT;
                    self.sprite_0_hit_cycle = None;
                }
            }

            if self.cycle == 256 {
                cart.step_scanline();
            }
        }

        // Scrolling updates during visible & pre-render scanlines
        if self.scanline < 240 || self.scanline == 261 {
            if self.cycle >= 1 && self.cycle <= 256 && (self.cycle & 7) == 0 {
                self.increment_scroll_x();
            }
            if self.cycle == 256 {
                self.increment_scroll_y();
            }
            if self.cycle == 257 {
                self.copy_scroll_x();
            }
        }

        // Post-render scanline (240): nothing happens

        // VBlank scanline (241)
        if self.scanline == 241 && self.cycle == 1 {
            self.status |= PpuStatus::VBLANK_STARTED;
            if (self.ctrl & PpuCtrl::GENERATE_NMI) != 0 {
                self.nmi_triggered = true;
            }
            self.frame_complete = true;
        }

        // Advance cycle and scanline
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.frame += 1;
            }
        }
    }

    fn render_scanline(&mut self, cart: &Cartridge) {
        self.render_background(cart);
        self.render_sprites(cart);
    }

    fn render_background(&mut self, cart: &Cartridge) {
        let y = self.scanline as usize;
        let show_bg = (self.mask & PpuMask::SHOW_BG) != 0;
        let show_bg_left = (self.mask & PpuMask::SHOW_BG_LEFT) != 0;
        let bg_pattern_base = if (self.ctrl & PpuCtrl::BG_PATTERN_ADDR) != 0 {
            0x1000
        } else {
            0x0000
        };

        let universal_bg_color = self.palette_ram[0];

        for x in 0..SCREEN_WIDTH {
            if !show_bg || (!show_bg_left && x < 8) {
                self.scanline_bg_color_indices[x] = 0;
                self.scanline_bg_palettes[x] = 0;

                let rgb = get_color_rgb(universal_bg_color);
                let idx = (y * SCREEN_WIDTH + x) * 3;
                self.frame_buffer[idx] = rgb.0;
                self.frame_buffer[idx + 1] = rgb.1;
                self.frame_buffer[idx + 2] = rgb.2;
                continue;
            }

            // Tile position based on scroll latched at start of scanline
            let fine_x = self.scanline_start_fine_x as u16;
            let coarse_x = (self.scanline_start_v & 0x001F) as usize;
            let coarse_y = ((self.scanline_start_v >> 5) & 0x001F) as usize;
            let fine_y = (self.scanline_start_v >> 12) & 0x07;
            let nt_select = ((self.scanline_start_v >> 10) & 0x03) as usize;

            // Compute pixel in tile
            let pixel_x = (x as u16) + fine_x;
            let tile_col = (coarse_x + (pixel_x as usize / 8)) % 64;
            let nt_x = (nt_select & 1) ^ ((tile_col / 32) & 1);
            let nt_y = ((nt_select >> 1) & 1) ^ ((coarse_y / 30) & 1);
            let current_nt = (nt_y << 1) | nt_x;
            let nt_col = (tile_col % 32) as u16;
            let nt_row = (coarse_y % 30) as u16;

            let nt_addr = 0x2000 | ((current_nt as u16) << 10) | (nt_row << 5) | nt_col;
            let tile_id = self.read_vram(nt_addr, cart) as u16;

            let tile_fine_y = fine_y;
            let tile_fine_x = 7 - (pixel_x % 8);

            let pattern_addr = bg_pattern_base + (tile_id * 16) + tile_fine_y;
            let p1 = self.read_vram(pattern_addr, cart);
            let p2 = self.read_vram(pattern_addr + 8, cart);

            let bit0 = (p1 >> tile_fine_x) & 1;
            let bit1 = (p2 >> tile_fine_x) & 1;
            let color_index = (bit1 << 1) | bit0;

            // Attribute byte (2x2 tile blocks = 16x16 pixels)
            let attr_addr =
                0x23C0 | ((current_nt as u16) << 10) | ((nt_row / 4) << 3) | (nt_col / 4);
            let attr_byte = self.read_vram(attr_addr, cart);
            let shift = ((nt_row & 2) << 1) | (nt_col & 2);
            let palette_id = (attr_byte >> shift) & 0x03;

            self.scanline_bg_color_indices[x] = color_index;
            self.scanline_bg_palettes[x] = palette_id;

            let palette_entry = if color_index == 0 {
                self.palette_ram[0]
            } else {
                let pal_ram_idx = ((palette_id << 2) | color_index) as usize;
                self.palette_ram[Self::palette_addr_mirror(0x3F00 + pal_ram_idx as u16)]
            };

            let rgb = get_color_rgb(palette_entry);
            let idx = (y * SCREEN_WIDTH + x) * 3;
            self.frame_buffer[idx] = rgb.0;
            self.frame_buffer[idx + 1] = rgb.1;
            self.frame_buffer[idx + 2] = rgb.2;
        }
    }

    fn render_sprites(&mut self, cart: &Cartridge) {
        if (self.mask & PpuMask::SHOW_SPRITES) == 0 {
            return;
        }

        let y = self.scanline as usize;
        let sprite_height = if (self.ctrl & PpuCtrl::SPRITE_SIZE) != 0 {
            16
        } else {
            8
        };
        let show_sprites_left = (self.mask & PpuMask::SHOW_SPRITES_LEFT) != 0;

        let mut sprite_count = 0;
        let mut sprites_on_scanline = [0usize; 8];

        for i in 0..64 {
            let sprite_y = self.oam[i * 4] as usize;
            if y >= sprite_y && y < sprite_y + sprite_height {
                if sprite_count < 8 {
                    sprites_on_scanline[sprite_count] = i;
                    sprite_count += 1;
                } else {
                    self.status |= PpuStatus::SPRITE_OVERFLOW;
                    break;
                }
            }
        }

        // Render from lowest priority to highest priority (index 7 down to 0)
        for &sprite_idx in sprites_on_scanline[..sprite_count].iter().rev() {
            let oam_offset = sprite_idx * 4;
            let sprite_y = self.oam[oam_offset] as usize;
            let tile_index = self.oam[oam_offset + 1] as u16;
            let attributes = self.oam[oam_offset + 2];
            let sprite_x = self.oam[oam_offset + 3] as usize;

            let flip_h = (attributes & 0x40) != 0;
            let flip_v = (attributes & 0x80) != 0;
            let behind_bg = (attributes & 0x20) != 0;
            let palette_id = (attributes & 0x03) + 4; // Sprite palettes are 4-7

            let mut row = y - sprite_y;
            if flip_v {
                row = sprite_height - 1 - row;
            }

            let pattern_addr = if sprite_height == 8 {
                let table_base = if (self.ctrl & PpuCtrl::SPRITE_PATTERN_ADDR) != 0 {
                    0x1000
                } else {
                    0x0000
                };
                table_base + (tile_index * 16) + (row as u16)
            } else {
                // 8x16 mode
                let table_base = if (tile_index & 1) != 0 {
                    0x1000
                } else {
                    0x0000
                };
                let tile = tile_index & !1;
                if row < 8 {
                    table_base + (tile * 16) + (row as u16)
                } else {
                    table_base + ((tile + 1) * 16) + ((row - 8) as u16)
                }
            };

            let p1 = self.read_vram(pattern_addr, cart);
            let p2 = self.read_vram(pattern_addr + 8, cart);

            for col in 0..8 {
                let pixel_col = if flip_h { col } else { 7 - col };
                let bit0 = (p1 >> pixel_col) & 1;
                let bit1 = (p2 >> pixel_col) & 1;
                let color_index = (bit1 << 1) | bit0;

                if color_index == 0 {
                    continue;
                }

                let x = sprite_x + col;
                if x >= SCREEN_WIDTH {
                    continue;
                }

                if !show_sprites_left && x < 8 {
                    continue;
                }

                // Sprite 0 hit detection: schedule hit at exact pixel cycle (x + 1)
                if sprite_idx == 0
                    && (self.status & PpuStatus::SPRITE_0_HIT) == 0
                    && self.sprite_0_hit_cycle.is_none()
                {
                    let bg_color_idx = self.scanline_bg_color_indices[x];
                    if bg_color_idx != 0 && x != 255 {
                        let bg_visible = (self.mask & PpuMask::SHOW_BG) != 0;
                        let bg_left_visible = (self.mask & PpuMask::SHOW_BG_LEFT) != 0;
                        if bg_visible && (x >= 8 || (show_sprites_left && bg_left_visible)) {
                            self.sprite_0_hit_cycle = Some((x as u16) + 1);
                        }
                    }
                }

                let bg_color_idx = self.scanline_bg_color_indices[x];
                if behind_bg && bg_color_idx != 0 {
                    continue;
                }

                let pal_ram_idx = ((palette_id << 2) | color_index) as usize;
                let palette_entry =
                    self.palette_ram[Self::palette_addr_mirror(0x3F00 + pal_ram_idx as u16)];
                let rgb = get_color_rgb(palette_entry);
                let idx = (y * SCREEN_WIDTH + x) * 3;
                self.frame_buffer[idx] = rgb.0;
                self.frame_buffer[idx + 1] = rgb.1;
                self.frame_buffer[idx + 2] = rgb.2;
            }
        }
    }
}
