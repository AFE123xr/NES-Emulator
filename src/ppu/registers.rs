pub struct PpuCtrl;

impl PpuCtrl {
    pub const NAMETABLE_1: u8 = 1 << 0;
    pub const NAMETABLE_2: u8 = 1 << 1;
    pub const VRAM_INCREMENT: u8 = 1 << 2; // 0: +1, 1: +32
    pub const SPRITE_PATTERN_ADDR: u8 = 1 << 3; // 0: $0000, 1: $1000
    pub const BG_PATTERN_ADDR: u8 = 1 << 4; // 0: $0000, 1: $1000
    pub const SPRITE_SIZE: u8 = 1 << 5; // 0: 8x8, 1: 8x16
    pub const MASTER_SLAVE: u8 = 1 << 6;
    pub const GENERATE_NMI: u8 = 1 << 7; // 0: off, 1: on
}

pub struct PpuMask;

impl PpuMask {
    pub const GREYSCALE: u8 = 1 << 0;
    pub const SHOW_BG_LEFT: u8 = 1 << 1;
    pub const SHOW_SPRITES_LEFT: u8 = 1 << 2;
    pub const SHOW_BG: u8 = 1 << 3;
    pub const SHOW_SPRITES: u8 = 1 << 4;
    pub const EMPHASIZE_RED: u8 = 1 << 5;
    pub const EMPHASIZE_GREEN: u8 = 1 << 6;
    pub const EMPHASIZE_BLUE: u8 = 1 << 7;
}

pub struct PpuStatus;

impl PpuStatus {
    pub const SPRITE_OVERFLOW: u8 = 1 << 5;
    pub const SPRITE_0_HIT: u8 = 1 << 6;
    pub const VBLANK_STARTED: u8 = 1 << 7;
}
