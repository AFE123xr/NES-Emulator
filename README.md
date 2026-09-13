# NES Emulator in Rust

A cycle-accurate, feature-complete Nintendo Entertainment System (NES / Famicom) emulator written entirely from scratch in Rust.

## Features

- **CPU (Ricoh 2A03 / MOS 6502)**:
  - Complete implementation of all 56 official 6502 instructions across all 13 addressing modes.
  - Comprehensive support for unofficial/undocumented opcodes (`LAX`, `SAX`, `DCP`, `ISC`, `SLO`, `RLA`, `SRE`, `RRA`, `ALR`, `ARR`, `AXS`, multi-byte `NOP`s, etc.).
  - Hardware bug emulation (e.g. indirect jump `JMP ($xxFF)` page-wrap anomaly).
  - Accurate cycle accounting including page-boundary crossings and branch penalties.
  - Passes 100% of the gold-standard **`nestest`** test suite (8,991 CPU states matched cycle-by-cycle against the reference log) and **Blargg's instruction tests**.
  - Interrupts: `RESET`, `NMI` (VBlank), and `IRQ` (APU frame counter, DMC, and MMC3 scanline IRQ).

- **PPU (Ricoh 2C02)**:
  - 256x240 display resolution at ~60.0988 FPS.
  - Full Loopy register scrolling model (`v`, `t`, `x`, `w`) with coarse and fine scroll manipulation.
  - Scanline-based background rendering with nametable mirroring (Vertical, Horizontal, Single Screen Lower/Upper, Four-Screen).
  - 8x8 and 8x16 sprite rendering with priority multiplexing, horizontal/vertical flipping, and sprite overflow detection.
  - Precise Sprite 0 hit detection.
  - Authentic 64-color NTSC system palette.

- **APU (Audio Processing Unit)**:
  - **Pulse 1 & Pulse 2**: Duty cycle generators (12.5%, 25%, 50%, 75%), volume envelope, sweep units (with 1's and 2's complement negation), and length counters.
  - **Triangle**: 32-step sequence generator, linear counter, length counter, and high-frequency click prevention.
  - **Noise**: 15-bit LFSR with 15-bit standard and 6-bit looped periodic modes, envelope, and length counter.
  - **DMC (Delta Modulation Channel)**: Direct memory DMA sample fetching, 7-bit DAC, frequency lookup table, and sample end IRQ.
  - **Mixer**: Non-linear synthesis formula and first-order low-pass and high-pass audio filters.
  - Real-time 44.1 kHz audio streaming via SDL2 audio callback.

- **Cartridge & Mappers**:
  - iNES (.nes) format parser with battery-backed PRG RAM support.
  - **Mapper 0 (NROM)**: *Super Mario Bros.*, *Donkey Kong*, *Pac-Man*, etc.
  - **Mapper 1 (MMC1)**: *The Legend of Zelda*, *Metroid*, *Mega Man 2*, *Kid Icarus*, etc.
  - **Mapper 2 (UxROM)**: *Castlevania*, *Contra*, *Mega Man*, *DuckTales*, etc.
  - **Mapper 3 (CNROM)**: *Adventure Island*, *Solomon's Key*, etc.
  - **Mapper 4 (MMC3)**: *Super Mario Bros. 3*, *Mega Man 3-6*, *Kirby's Adventure*, etc. with scanline IRQ counter.
  - **Mapper 7 (AxROM)**: *Battletoads*, *Marble Madness*, etc.

- **Frontend & Controls**:
  - SDL2 hardware-accelerated rendering with configurable window scaling (1x–8x) and fullscreen mode.
  - Smooth 60 FPS frame pacing.
  - Headless benchmark mode (>1,000 FPS on modern CPUs).

---

## Controls & Hotkeys

### Player 1 Gamepad
| Action | Primary Key | Secondary Key |
|---|---|---|
| **D-Pad Up** | Up Arrow | `W` |
| **D-Pad Down** | Down Arrow | `S` |
| **D-Pad Left** | Left Arrow | `A` |
| **D-Pad Right** | Right Arrow | `D` |
| **A Button** | `Z` | `K` |
| **B Button** | `X` | `J` |
| **Select** | Space | Right Shift |
| **Start** | Enter | Return |

### Hotkeys
| Key | Action |
|---|---|
| `F12`, `F2` | Capture Screenshot (saved to `./screenshots/` in lossless 24-bit BMP) |
| `F3` | Toggle real-time Audio Diagnostics (buffer health, underruns, pop detection) |
| `P` | Pause / Resume emulation |
| `R` | Reset console |
| `Tab` | Fast Forward (hold for uncapped speed) |
| `M` | Mute / Unmute audio |
| `F11` | Toggle Fullscreen |
| `Esc` | Quit emulator |

---

## Building and Running

### Prerequisites
- **Rust**: Latest stable Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **SDL2**:
  - **macOS**: `brew install sdl2`
  - **Ubuntu / Debian**: `sudo apt install libsdl2-dev`
  - **Fedora**: `sudo dnf install SDL2-devel`
  - **Arch Linux**: `sudo pacman -S sdl2`

### Build
```bash
cargo build --release
```
The optimized executable will be located at `target/release/nes`.

### Run a ROM
```bash
# Launch a game at default 3x scale
./target/release/nes path/to/game.nes

# Launch with 4x scale
./target/release/nes path/to/game.nes --scale 4

# Launch with real-time audio diagnostics enabled
./target/release/nes path/to/game.nes --debug-audio

# Run in headless benchmark mode for 1000 frames
./target/release/nes path/to/game.nes --headless 1000

# Or launch using the ROM_PATH / NES_ROM environment variable
export ROM_PATH=path/to/game.nes
./target/release/nes
```

---

## Running Tests

The test suite includes full CPU golden-master verification against `nestest.log`, Blargg's CPU instruction tests, PPU mirroring and register tests, APU audio channel tests, and mapper switching tests:

```bash
cargo test
```

---

## Project Structure

```
├── build.rs                  # Native library linking configuration
├── Cargo.toml                # Project configuration & dependencies
├── src/
│   ├── lib.rs                # Library entrypoint & module exports
│   ├── main.rs               # Executable entrypoint & CLI argument handling
│   ├── apu/                  # Audio Processing Unit
│   │   ├── dmc.rs            # Delta Modulation Channel (DMC)
│   │   ├── envelope.rs       # Volume envelope generator
│   │   ├── filter.rs         # Low-pass and high-pass audio filters
│   │   ├── length_counter.rs # Note length counter
│   │   ├── noise.rs          # Noise channel with LFSR
│   │   ├── pulse.rs          # Pulse channels with frequency sweep
│   │   └── triangle.rs       # Triangle channel with linear counter
│   ├── bus/                  # System interconnect bus & memory mapping
│   ├── cartridge/            # ROM parser & Mappers
│   │   ├── header.rs         # iNES header decoder
│   │   └── mapper/           # Mappers 0, 1, 2, 3, 4, 7
│   ├── controller/           # NES joypad shift registers & strobe logic
│   ├── cpu/                  # Ricoh 2A03 / MOS 6502 core
│   │   ├── addressing.rs     # 13 addressing modes & page crossing logic
│   │   ├── opcodes.rs        # Complete 256 opcode table
│   │   └── registers.rs      # Status flags and registers
│   ├── nes/                  # Master console coordination & clock stepping
│   ├── ppu/                  # Ricoh 2C02 Picture Processing Unit
│   │   ├── palette.rs        # Standard NES 64-color palette
│   │   └── registers.rs      # PPU status and control flags
│   └── ui/                   # SDL2 window, audio device, and event loop
└── tests/                    # Integration & validation tests
    ├── nestest.rs            # Automated nestest golden master test
    ├── blargg_tests.rs       # Blargg instruction test
    ├── ppu_tests.rs          # PPU VRAM mirroring and register tests
    ├── apu_tests.rs          # APU channel tests
    └── mapper_tests.rs       # Mapper banking & IRQ tests
```
