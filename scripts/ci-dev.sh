#!/usr/bin/env bash
#
# ci-dev.sh - Local CI/Dev verification script for NES Emulator.
#
# Verifies:
# 1. Code formatting (`cargo fmt --check`)
# 2. Compilation of all targets and binaries (`cargo check --all-targets`)
# 3. Static analysis & lints (`cargo clippy --all-targets -- -D warnings`)
# 4. Release binary compilation (`cargo build --release`)
# 5. Unit and fixture test suites (without requiring external ROM downloads)
# 6. CLI invocation and headless execution
#

set -euo pipefail

# ANSI color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=======================================${NC}"
echo -e "${BLUE}   NES Emulator - CI-Dev Verification  ${NC}"
echo -e "${BLUE}=======================================${NC}"

# Step 1: Check formatting
echo -e "\n${YELLOW}[1/6] Checking code formatting (cargo fmt)...${NC}"
if cargo fmt --all -- --check; then
    echo -e "${GREEN}✓ Formatting check passed.${NC}"
else
    echo -e "${RED}✗ Formatting check failed. Run 'cargo fmt' to fix.${NC}"
    exit 1
fi

# Step 2: Check compilation
echo -e "\n${YELLOW}[2/6] Checking compilation of binary and library targets (cargo check)...${NC}"
cargo check --all-targets
echo -e "${GREEN}✓ Compilation check passed.${NC}"

# Step 3: Run Clippy lints
echo -e "\n${YELLOW}[3/6] Running Clippy linter (cargo clippy)...${NC}"
cargo clippy --all-targets -- -D warnings
echo -e "${GREEN}✓ Clippy passed with 0 warnings.${NC}"

# Step 4: Build release binary
echo -e "\n${YELLOW}[4/6] Building release binary (cargo build --release)...${NC}"
cargo build --release
echo -e "${GREEN}✓ Release binary built successfully at target/release/nes.${NC}"

# Step 5: Run unit tests and committed fixture tests (no downloaded ROMs needed)
echo -e "\n${YELLOW}[5/6] Running unit tests and fixture tests...${NC}"
# Unset any local external ROM path env vars so tests run in pure CI-mode
env -u ROM_PATH -u NES_ROM -u NES_ROMS_DIR cargo test -- --nocapture
echo -e "${GREEN}✓ All unit and fixture tests passed.${NC}"

# Step 6: Verify headless execution with built release binary
echo -e "\n${YELLOW}[6/6] Verifying CLI and headless execution...${NC}"
./target/release/nes --help > /dev/null
./target/release/nes tests/fixtures/nestest.nes --headless 100
echo -e "${GREEN}✓ Headless binary execution verified.${NC}"

echo -e "\n${GREEN}=======================================${NC}"
echo -e "${GREEN}   ✓ ALL CI-DEV CHECKS PASSED!        ${NC}"
echo -e "${GREEN}=======================================${NC}"
