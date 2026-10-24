# 🎮 NES Emulator in Rust

[![CI](https://github.com/AFE123xr/NES-Emulator/actions/workflows/ci.yml/badge.svg)](https://github.com/AFE123xr/NES-Emulator/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests](https://img.shields.io/badge/tests-26%20passed%20(100%25)-success.svg)](#testing--verification)
[![Cycle Accurate](https://img.shields.io/badge/CPU-cycle--accurate%20nestest%20match-blue.svg)](#cpu-ricoh-2a03--mos-6502)
[![Speed](https://img.shields.io/badge/benchmark->750%20FPS%20headless-orange.svg)](#performance--benchmarking)

A high-performance, cycle-accurate Nintendo Entertainment System (NES / Famicom) emulator written completely from scratch in pure **Rust**, featuring pixel-perfect PPU rendering, a high-fidelity 5-channel APU with real-time audio diagnostics and WAV recording, comprehensive mapper support (NROM, MMC1, UxROM, CNROM, MMC3, AxROM), and hardware-accelerated SDL2 frontend.

---

## 📸 Screenshot Showcase

Captured directly from the emulator's PPU scanline pipeline:

| **Super Mario Bros.** (World 1-1 Gameplay) | **The Legend of Zelda** (Title Screen) |
| :---: | :---: |
| <img src="images/mario_gameplay.png" width="370" alt="Super Mario Bros." /> | <img src="images/zelda_title.png" width="370" alt="The Legend of Zelda" /> |
| *Mapper 0 (NROM) • Fine scrolling & sprite multiplexing* | *Mapper 1 (MMC1) • Scanline split & Sprite 0 timing* |

| **Mega Man 2** (Title Screen) | **Contra** (Title Screen) |
| :---: | :---: |
| <img src="images/megaman2_title.png" width="370" alt="Mega Man 2" /> | <img src="images/contra_title.png" width="370" alt="Contra" /> |
| *Mapper 1 (MMC1) • Full 8x16 sprite rendering* | *Mapper 2 (UxROM) • Dynamic PRG bank switching* |

| **Castlevania** (Title Screen) | **Super Mario Bros. 3** (Title Screen) |
| :---: | :---: |
| <img src="images/castlevania_title.png" width="370" alt="Castlevania" /> | <img src="images/smb3_title.png" width="370" alt="Super Mario Bros. 3" /> |
| *Mapper 2 (UxROM) • Background tile rendering* | *Mapper 4 (MMC3) • Scanline IRQ split screen* |

| **Zelda II - The Adventure of Link** | **Super Mario Bros.** (Title Screen) |
| :---: | :---: |
| <img src="images/zelda2_title.png" width="370" alt="Zelda II" /> | <img src="images/mario_title.png" width="370" alt="Super Mario Bros. Title" /> |
| *Mapper 1 (MMC1) • Battery-backed SRAM saving* | *Mapper 0 (NROM) • Classic 64-color NTSC palette* |

---

## ⚡ Key Highlights & Architecture

### 🧠 CPU (Ricoh 2A03 / MOS 6502)
- **100% Instruction Coverage**: Implements all 56 official 6502 instructions across all 13 addressing modes.
- **Unofficial Opcodes**: Full support for undocumented opcodes (`LAX`, `SAX`, `DCP`, `ISC`, `SLO`, `RLA`, `SRE`, `RRA`, `ALR`, `ARR`, `AXS`, multi-byte `NOP`s, etc.).
- **Hardware-Accurate Quirks**: Faithful replication of hardware edge cases, including indirect jump `JMP ($xxFF)` page-wrap anomaly and cycle-5 RMW dummy memory writes (critical for MMC1 consecutive-cycle write filtering in *The Legend of Zelda*).
- **Golden Master Verification**: Passes **100% of the `nestest` gold standard** (all 8,991 CPU instructions and cycles matched cycle-by-cycle against reference logs) and **Blargg's official instruction tests**.
- **Interrupt Handling**: Exact cycle timing for `RESET`, `NMI` (VBlank), and `IRQ` (APU frame counter, DMC DMA, and MMC3 scanline counter).

### 🎨 PPU (Ricoh 2C02 Picture Processing Unit)
- **Resolution & Timing**: Native 256×240 display resolution at ~60.0988 FPS (29,780.5 CPU cycles per frame).
- **Loopy Scrolling**: Full register architecture (`v`, `t`, `x`, `w`) with mid-scanline coarse and fine scroll manipulation.
- **Nametable Mirroring**: Dynamic hardware mirroring switching (Horizontal, Vertical, Single Screen Lower/Upper, and Four-Screen).
- **Sprite Multiplexing**: Supports both 8×8 and 8×16 sprite modes with priority sorting, horizontal and vertical flipping, and sprite overflow flag.
- **Precise Sprite 0 Hit**: Cycle-accurate evaluation for split-screen HUD status bars (e.g. *Super Mario Bros.* and *The Legend of Zelda* overworld).
- **NTSC Palette**: Authentic, vibrant 64-color composite video palette.

### 🔊 APU (Audio Processing Unit & Diagnostics)
- **5 Complete Audio Channels**:
  - **Pulse 1 & Pulse 2**: 4 duty cycles (12.5%, 25%, 50%, 75%), volume envelope generator, and sweep units (Pulse 1 with 1's complement negation, Pulse 2 with 2's complement negation, with continuous muting evaluation).
  - **Triangle**: 32-step pseudo-triangle wave, linear counter, length counter, and pop-free ultrasonic frequency handling.
  - **Noise**: 15-bit Linear Feedback Shift Register (LFSR) with 32,767-step pseudo-random and 93-step periodic looped modes, clocked at true CPU cycle periods.
  - **DMC (Delta Modulation Channel)**: Automatic 1-bit delta DMA sample playback, 7-bit DAC, frequency lookup table, and sample end IRQ.
- **Hardware Filters & Mixing**: Non-linear DAC synthesis formula, first-order low-pass filter (14 kHz), and dual first-order high-pass filters (90 Hz & 440 Hz) for DC offset removal.
- **Frame Sequencer**: Hardware 4-step (240 Hz / 120 Hz / 60 Hz IRQ) and 5-step sequences clocked accurately on APU cycles.
- **Audio-Driven Frame Pacing & DRC**: Dynamic Rate Control smoothly synchronizes emulation speed to the physical soundcard DAC, preventing buffer underruns.
- **Audio Diagnostics & `.wav` Export**:
  - Rolling 2-minute audio buffer recorded in real-time.
  - **F4 Hotkey**: Instantly dumps recent audio to a standard 16-bit 44.1 kHz PCM WAV file in `./audio_dumps/`.
  - **Optional Auto-save on Exit**: Pass `--save-audio-on-exit` to write the audio session to a `.wav` file upon closing the emulator for easy analysis.

### 💾 Cartridge & Mapper Support
Full iNES format parsing with battery-backed PRG RAM persistence:
| Mapper | Chip | Popular Supported Games |
|:---:|:---:|:---|
| **0** | **NROM** | *Super Mario Bros.*, *Donkey Kong*, *Pac-Man*, *Excitebike*, *Kung Fu* |
| **1** | **MMC1** | *The Legend of Zelda*, *Metroid*, *Mega Man 2*, *Zelda II*, *Kid Icarus* |
| **2** | **UxROM** | *Castlevania*, *Contra*, *Mega Man*, *DuckTales*, *Metal Gear* |
| **3** | **CNROM** | *Adventure Island*, *Solomon's Key*, *Gradius*, *Cyber Stadium Series* |
| **4** | **MMC3** | *Super Mario Bros. 3*, *Mega Man 3–6*, *Kirby's Adventure*, *Super C* |
| **7** | **AxROM** | *Battletoads*, *Marble Madness*, *Double Dragon II* |

---

## 🎮 Controls & Keybindings

All keyboard controls and hotkeys can be fully customized via a JSON configuration file. When starting, the emulator automatically searches for `./controls.json`. You can also specify an arbitrary config file with `--controls <path>` or the `NES_CONTROLS` environment variable.

### Default Gamepad Controls

| Button | Player 1 Primary | Player 1 Secondary | Player 2 |
|:---|:---|:---|:---|
| **D-Pad Up** | Up Arrow (`↑`) | `W` | `I` |
| **D-Pad Down** | Down Arrow (`↓`) | `S` | `K` |
| **D-Pad Left** | Left Arrow (`←`) | `A` | `J` |
| **D-Pad Right** | Right Arrow (`→`) | `D` | `L` |
| **A Button** | `Z` | `K` | `O` |
| **B Button** | `X` | `J` | `U` |
| **Select** | `Space` | Right Shift | `Y` |
| **Start** | `Enter` | Return | `H` |

### Emulator Hotkeys

| Key | Function |
|:---|:---|
| **`F12` / `F2`** | **Capture Screenshot** (lossless 24-bit BMP saved to `./screenshots/`) |
| **`F4`** | **Save Audio Dump** (lossless 16-bit 44.1 kHz `.wav` saved to `./audio_dumps/`) |
| **`F3`** | **Toggle Audio Diagnostics** (real-time buffer health, underrun & pop monitor) |
| **`F8`** | **Toggle Cheats** On / Off |
| **`P`** | **Pause / Resume** emulation |
| **`R`** | **Reset** console |
| **`Tab`** | **Fast-Forward** (hold for uncapped speed, up to >750 FPS) |
| **`M`** | **Mute / Unmute** audio |
| **`F11`** | **Toggle Fullscreen** mode |
| **`Esc`** | **Quit Emulator** (saves audio dump only with `--save-audio-on-exit`) |

### 🛠️ Customizing Controls with JSON

You can configure any button to use a single key name (e.g. `"W"`) or multiple keys in an array (e.g. `["Up", "W"]`).

Generate a clean default configuration file anytime using:
```bash
# Generate controls.json in current directory
./target/release/nes --generate-controls

# Or generate to a custom path
./target/release/nes --generate-controls my_controls.json
```

Example [`controls.json`](controls.json):
```json
{
  "player1": {
    "up": ["Up", "W"],
    "down": ["Down", "S"],
    "left": ["Left", "A"],
    "right": ["Right", "D"],
    "a": ["Z", "K"],
    "b": ["X", "J"],
    "select": ["Space", "RShift"],
    "start": "Return"
  },
  "player2": {
    "up": "I",
    "down": "K",
    "left": "J",
    "right": "L",
    "a": "O",
    "b": "U",
    "select": "Y",
    "start": "H"
  },
  "hotkeys": {
    "pause": "P",
    "reset": "R",
    "fast_forward": "Tab",
    "mute": "M",
    "audio_diagnostics": "F3",
    "audio_dump": "F4",
    "screenshot": ["F12", "F2"],
    "fullscreen": "F11",
    "exit": "Escape"
  }
}
```

#### Supported Key Names & Aliases
- **Letters**: `"A"`–`"Z"` (case-insensitive, e.g. `"w"` or `"W"`)
- **Numbers**: `"0"`–`"9"`, `"Numpad0"`–`"Numpad9"` (or `"Kp0"`–`"Kp9"`)
- **Directions**: `"Up"`, `"Down"`, `"Left"`, `"Right"` (or `"ArrowUp"`, etc.)
- **Navigation & Editing**: `"Return"`, `"Enter"`, `"Space"`, `"Tab"`, `"Backspace"`, `"Escape"` / `"Esc"`, `"Insert"`, `"Delete"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`
- **Modifiers**: `"LShift"`, `"RShift"`, `"Shift"`, `"LCtrl"`, `"RCtrl"`, `"Ctrl"`, `"LAlt"`, `"RAlt"`, `"Alt"`
- **Function Keys**: `"F1"` through `"F12"`
- **Symbols**: `"Period"` / `"."`, `"Comma"` / `","`, `"Minus"` / `"-"`, `"Plus"` / `"+"`, `"Equals"` / `"="`, `"Slash"` / `"/"`, `"Backslash"` / `"\\"`

### ✨ Memory Cheats Configuration (`cheats.json`)

The emulator automatically loads memory cheats from [`cheats.json`](cheats.json) in the working directory (or from a custom path via `--cheats <file>` or the `NES_CHEATS` environment variable). Cheats can also be embedded directly inside `controls.json` under a `"cheats"` array, and toggled at any time in-game with **`F8`**.

Example [`cheats.json`](cheats.json) (configured for *Contra* Spread Gun + Rapid Fire):
```json
{
  "cheats": [
    {
      "name": "Contra - Player 1 Spread Gun + Rapid Fire",
      "address": "0x00AA",
      "value": "0x13",
      "enabled": true
    },
    {
      "name": "Contra - Player 2 Spread Gun + Rapid Fire",
      "address": "0x00AB",
      "value": "0x13",
      "enabled": true
    }
  ]
}
```
- **`address`**: 16-bit CPU memory address in hex (`"0x00AA"`, `"$00AA"`) or decimal (`170`).
- **`value`**: 8-bit value to write/freeze in hex (`"0x13"`, `"$13"`) or decimal (`19`). In *Contra*, `0x00AA` (P1) and `0x00AB` (P2) store the active weapon in the low nibble (`0x03` = Spread Gun) and rapid fire in bit 4 (`0x10` = Rapid Fire), so `0x13` grants Spread Gun with Rapid Fire.
- **`compare`** *(optional)*: Only apply the cheat when the current value at `address` equals `compare`.
- **`enabled`** *(optional, default `true`)*: Enable or disable individual cheats.

---

## 🚀 Getting Started

### Prerequisites

- **Rust**: Latest stable toolchain ([Install via rustup](https://rustup.rs/)):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **SDL2 Development Libraries**:
  - **macOS** (Homebrew):
    ```bash
    brew install sdl2
    ```
  - **Ubuntu / Debian**:
    ```bash
    sudo apt update && sudo apt install libsdl2-dev
    ```
  - **Fedora / RHEL**:
    ```bash
    sudo dnf install SDL2-devel
    ```
  - **Arch Linux**:
    ```bash
    sudo pacman -S sdl2
    ```

### Build

```bash
cargo build --release
```
The optimized standalone binary will be compiled to `target/release/nes`.

---

## 🕹️ Running Games

Run any standard `.nes` ROM file directly by providing the path:

```bash
# Launch a game at default 3x window scaling (768×720)
./target/release/nes path/to/game.nes

# Launch at 4x window scaling (1024×960)
./target/release/nes path/to/game.nes --scale 4

# Launch with real-time audio diagnostics enabled
./target/release/nes path/to/game.nes --debug-audio

# Run in headless benchmark mode for 1,000 frames
./target/release/nes path/to/game.nes --headless 1000

# View all CLI options and keybindings
./target/release/nes --help
```

### CLI Options

| Flag | Argument | Description | Example |
|:---|:---|:---|:---|
| `<rom_path>` | File path | Path to any valid iNES ROM file | `./target/release/nes roms/mario.nes` |
| `--host` | `[port]` | Host a P2P multiplayer session as Player 1 (default UDP port: `7878`) and print shareable hash | `--host` or `--host 9000` |
| `--host-ip` | `<ip>` | Override the IP address encoded in the host's shareable hash (e.g. public IP or VPN/Tailscale IP) | `--host-ip 100.64.1.25` |
| `--join` | `<hash>` | Join a P2P multiplayer session as Player 2 using the host's custom hash code | `--join NES-XXXXXX-XXXXXX-XXXXXX-XXXXXX` |
| `--delay` | `1`–`10` | Lockstep input delay in frames when hosting (default: `2`) | `--delay 3` |
| `--ignore-rom-hash` | *None* | Allow P2P connection even if ROM checksums differ (e.g. translated/patched ROMs) | `--ignore-rom-hash` |
| `--controls`, `--config` | `<file>` | Path to custom controls JSON file (default: `./controls.json`) | `--controls custom_keys.json` |
| `--generate-controls` | `[file]` | Generate a default controls JSON configuration file | `--generate-controls controls.json` |
| `--cheats` | `<file>` | Path to custom cheats JSON file (default: `./cheats.json`) | `--cheats cheats.json` |
| `--no-cheats` | *None* | Disable all cheats for the current session | `--no-cheats` |
| `--generate-cheats` | `[file]` | Generate a default cheats JSON file with Contra Spread + Rapid Fire | `--generate-cheats cheats.json` |
| `--scale` | `1`–`8` | Window scale multiplier (default: `3` = 768×720) | `--scale 4` (1024×960 window) |
| `--debug-audio` | *None* | Enable real-time audio buffer and underrun diagnostics | `--debug-audio` |
| `--save-audio-on-exit` | *None* | Save the session audio as a `.wav` to `./audio_dumps/` on exit (off by default) | `--save-audio-on-exit` |
| `--headless` | `<frames>` | Run headlessly for $N$ frames without a window (benchmarking/testing) | `--headless 1000` |
| `--help`, `-h` | *None* | Display usage instructions and control bindings | `--help` |

---

## 🌐 P2P Online Multiplayer (Custom Shareable Hash)

Play 2-player NES classics (e.g. *Contra*, *Super Mario Bros.*, *Double Dragon II*, *Bubble Bobble*) over a direct UDP peer-to-peer connection using a compact custom invite hash (`NES-XXXXXX-XXXXXX-XXXXXX-XXXXXX`).

### How It Works
- **Custom Shareable Hash (`InviteHash`)**: Encodes the host's IP address (IPv4 or IPv6), UDP port, 32-bit FNV-1a ROM checksum, 16-bit session token, negotiated frame input delay, and an 8-bit CRC-8 integrity check (with XOR keystream obfuscation) into a human-friendly Crockford Base32 code.
- **Deterministic Frame Lockstep**: Host (Player 1) and Peer (Player 2) exchange controller inputs for frame `F + delay` along with a rolling 16-frame redundant input history (preventing single UDP packet drops from causing stalls) and per-frame emulator state checksums (`CPU + RAM + PPU`) to verify zero-desync synchronization.
- **Synchronized Console Reset & Pause**: Pressing `R` (Reset) or `P` (Pause) schedules a frame-locked control flag so both emulators execute the reset or pause on the exact same frame.
- **Ergonomic Controls**: In P2P mode, both Player 1 (Host) and Player 2 (Peer) use standard Player 1 keybindings (Arrow keys / WASD, Z/K, X/J, Space, Enter) on their own keyboard.

### Starting a Multiplayer Game

1. **Player 1 (Host)** launches the game with `--host`:
   ```bash
   ./target/release/nes path/to/contra.nes --host
   ```
   The emulator prints a shareable invite hash to the terminal (and displays it in the window title while waiting):
   ```text
   Shareable Hash:    NES-7K4M9P-2X8QR5-T1W9B3-N6C4H8
   Localhost Hash:    NES-3F9A1C-8D2E4B-7M5P0Q-W6X9Z2 (same machine)
   ```
   *(Tip: If hosting across the internet via port-forwarding or a VPN/Tailscale IP, pass `--host-ip <your_ip>` and optionally `--delay 3`.)*

2. **Player 2 (Peer)** launches the same ROM with `--join <hash>`:
   ```bash
   ./target/release/nes path/to/contra.nes --join NES-7K4M9P-2X8QR5-T1W9B3-N6C4H8
   ```
   As soon as Player 2 connects, both emulators synchronize at frame 0 and display live ping (`ms`) and frame delay in the window title bar.

---

## 🧪 Testing & Verification

The project enforces strict quality control through automated continuous integration (CI) and a comprehensive test suite covering CPU instruction accuracy, PPU rendering, APU audio synthesis, and mapper banking.

### Local CI-Dev Verification
Run the complete CI check locally with one command:
```bash
./scripts/ci-dev.sh
```
This automated pipeline runs:
1. **Formatting Check**: Verifies adherence to standard Rust style (`cargo fmt --check`).
2. **Compilation Check**: Validates that all library targets and binary entrypoints compile cleanly (`cargo check --all-targets`).
3. **Linter**: Enforces zero warnings with Clippy (`cargo clippy --all-targets -- -D warnings`).
4. **Release Build**: Compiles the optimized release binary (`cargo build --release`).
5. **ROM-Free Unit & Fixture Tests**: Runs all unit tests without requiring external ROM downloads (`cargo test`).
6. **Headless Execution Check**: Confirms CLI functionality and headless execution on `nestest.nes`.

### Running Tests Manually
```bash
# Run all unit tests and committed fixture tests (no downloaded ROMs required)
cargo test

# Run tests with live output
cargo test -- --nocapture

# Run cycle-accurate golden master CPU verification
cargo test --test nestest

# Run Blargg's official instruction tests
cargo test --test blargg_tests

# Run APU sound channel tests
cargo test --test apu_tests

# Run PPU registers and scrolling tests
cargo test --test ppu_tests

# Run mapper banking and IRQ tests
cargo test --test mapper_tests
```

### Verification Highlights
- **No External ROMs Required for CI**: Unit tests and integration tests use pure in-memory data or committed fixtures in `tests/fixtures/` (`nestest.nes`, `nestest.log`, `official_only.nes`). Optional tests requiring copyrighted commercial ROMs cleanly skip when `ROM_PATH` / `NES_ROM` is unset.
- **`nestest.nes`**: Matches all 8,991 CPU instructions and cycles against `nestest.log` golden master.
- **Blargg's `official_only.nes`**: All 56 official 6502 instructions pass.
- **Zero Warnings Policy**: Enforces clean `cargo clippy --all-targets` and `cargo fmt --check`.

---

## 🏎️ Performance & Benchmarking

Benchmarked on Apple Silicon (M-series) in release mode:
- **Headless Mode**: **748.9 FPS** (~12.5× real-time speed).
- **Interactive Mode**: Rock-solid **60.0988 FPS** with low-latency audio playback and sub-frame input response.

---

## 📁 Project Structure

```text
NES-Emulator/
├── .github/
│   └── workflows/
│       └── ci.yml                # Automated GitHub Actions CI workflow
├── scripts/
│   └── ci-dev.sh                 # Local CI-dev test & verification runner
├── controls.json                 # Configurable JSON controls template
├── Cargo.toml                    # Rust crate configuration & dependencies
├── build.rs                      # Native SDL2 library linking
├── README.md                     # Documentation and screenshots
├── images/                       # High-res screenshot showcase assets
├── screenshots/                  # In-game captures (F12/F2)
├── audio_dumps/                  # In-game WAV audio recordings (F4 / exit)
├── src/
│   ├── lib.rs                    # Public API exports
│   ├── main.rs                   # CLI arguments & entrypoint
│   ├── apu/                      # Audio Processing Unit
│   │   ├── dmc.rs                # Delta Modulation Channel (DMC)
│   │   ├── envelope.rs           # Volume envelope generator
│   │   ├── filter.rs             # 14 kHz low-pass & 90/440 Hz high-pass filters
│   │   ├── length_counter.rs     # Note length counters
│   │   ├── noise.rs              # 15-bit LFSR pseudo-random noise
│   │   ├── pulse.rs              # Pulse channels with frequency sweep
│   │   └── triangle.rs           # 32-step triangle wave channel
│   ├── audio/                    # SDL2 audio callback & diagnostics
│   │   ├── mod.rs                # Dynamic rate control & audio streaming
│   │   └── diagnostics.rs        # WAV exporter, buffer monitor & pop detection
│   ├── bus/                      # 16-bit CPU system interconnect bus
│   ├── cartridge/                # Cartridge header decoder & mappers
│   │   ├── header.rs             # iNES header parser
│   │   └── mapper/               # Mappers 0, 1, 2, 3, 4, 7
│   ├── controller/               # Standard NES joypad & JSON input configuration
│   │   ├── mod.rs                # Joypad shift register logic
│   │   └── config.rs             # JSON controls parsing & InputMap bindings
│   ├── cpu/                      # Ricoh 2A03 / MOS 6502 CPU core
│   │   ├── addressing.rs         # 13 addressing modes
│   │   ├── opcodes.rs            # Official & unofficial opcodes
│   │   └── registers.rs          # Status flags and registers
│   ├── nes/                      # Console master coordination & clock stepping
│   ├── ppu/                      # Ricoh 2C02 Picture Processing Unit
│   │   ├── palette.rs            # 64-color authentic NTSC palette
│   │   └── registers.rs          # Loopy scrolling & status registers
│   └── ui/                       # SDL2 window, event loop & input handling
│       └── screenshot.rs         # Lossless 24-bit BMP screenshot exporter
└── tests/                        # Comprehensive test suite
    ├── apu_tests.rs              # Audio channel unit tests
    ├── blargg_tests.rs           # Blargg CPU instruction tests
    ├── capture_readme_screenshots.rs # Automated screenshot capture pipeline
    ├── controls_test.rs          # JSON controls configuration & key mapping tests
    ├── mapper_tests.rs           # Mappers 0, 1, 2, 4 IRQ tests
    ├── nestest.rs                # Golden master nestest cycle verification
    ├── ppu_tests.rs              # PPU scrolling & VRAM mirroring tests
    ├── screenshot_tests.rs       # BMP export format regression test
    ├── zelda_audio_test.rs       # Zelda audio waveform & pop analysis
    ├── zelda_debug.rs            # Zelda menu navigation & scrolling test
    └── zelda_sprite0.rs          # Zelda overworld Sprite 0 split-timing test
```

---

## 📜 License

This project is licensed under the [MIT License](LICENSE).
