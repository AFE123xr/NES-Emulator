pub mod addressing;
pub mod opcodes;
pub mod registers;

use addressing::AddressingMode;
use opcodes::get_opcode;
pub use registers::{CpuFlags, Registers};

pub trait Memory {
    fn mem_read(&mut self, addr: u16) -> u8;
    fn mem_write(&mut self, addr: u16, data: u8);
    fn mem_write_cycle(&mut self, addr: u16, data: u8, cycle: u64) {
        let _ = cycle;
        self.mem_write(addr, data);
    }

    fn mem_read_u16(&mut self, addr: u16) -> u16 {
        let lo = self.mem_read(addr) as u16;
        let hi = self.mem_read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    fn mem_write_u16(&mut self, addr: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xFF) as u8;
        self.mem_write(addr, lo);
        self.mem_write(addr.wrapping_add(1), hi);
    }
}

pub struct Cpu {
    pub registers: Registers,
    pub total_cycles: u64,
    pub nmi_pending: bool,
    pub irq_pending: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            total_cycles: 0,
            nmi_pending: false,
            irq_pending: false,
        }
    }

    pub fn reset(&mut self, bus: &mut impl Memory) {
        self.registers.sp = 0xFD;
        self.registers.p = CpuFlags::INTERRUPT_DISABLE | CpuFlags::UNUSED;
        self.registers.pc = bus.mem_read_u16(0xFFFC);
        self.total_cycles = 7;
        self.nmi_pending = false;
        self.irq_pending = false;
    }

    pub fn trigger_nmi(&mut self) {
        self.nmi_pending = true;
    }

    pub fn set_irq(&mut self, active: bool) {
        self.irq_pending = active;
    }

    pub fn stack_push(&mut self, bus: &mut impl Memory, val: u8) {
        let addr = 0x0100 + self.registers.sp as u16;
        bus.mem_write(addr, val);
        self.registers.sp = self.registers.sp.wrapping_sub(1);
    }

    pub fn stack_pop(&mut self, bus: &mut impl Memory) -> u8 {
        self.registers.sp = self.registers.sp.wrapping_add(1);
        let addr = 0x0100 + self.registers.sp as u16;
        bus.mem_read(addr)
    }

    pub fn stack_push_u16(&mut self, bus: &mut impl Memory, val: u16) {
        self.stack_push(bus, (val >> 8) as u8);
        self.stack_push(bus, (val & 0xFF) as u8);
    }

    pub fn stack_pop_u16(&mut self, bus: &mut impl Memory) -> u16 {
        let lo = self.stack_pop(bus) as u16;
        let hi = self.stack_pop(bus) as u16;
        (hi << 8) | lo
    }

    fn handle_nmi(&mut self, bus: &mut impl Memory) {
        self.stack_push_u16(bus, self.registers.pc);
        let p_to_push = (self.registers.p & !CpuFlags::BREAK) | CpuFlags::UNUSED;
        self.stack_push(bus, p_to_push);
        self.registers.set_flag(CpuFlags::INTERRUPT_DISABLE, true);
        self.registers.pc = bus.mem_read_u16(0xFFFA);
        self.total_cycles += 7;
    }

    fn handle_irq(&mut self, bus: &mut impl Memory) {
        self.stack_push_u16(bus, self.registers.pc);
        let p_to_push = (self.registers.p & !CpuFlags::BREAK) | CpuFlags::UNUSED;
        self.stack_push(bus, p_to_push);
        self.registers.set_flag(CpuFlags::INTERRUPT_DISABLE, true);
        self.registers.pc = bus.mem_read_u16(0xFFFE);
        self.total_cycles += 7;
    }

    pub fn step(&mut self, bus: &mut impl Memory) -> u8 {
        if self.nmi_pending {
            self.nmi_pending = false;
            self.handle_nmi(bus);
            return 7;
        }

        if self.irq_pending && !self.registers.get_flag(CpuFlags::INTERRUPT_DISABLE) {
            self.handle_irq(bus);
            return 7;
        }

        let opcode_byte = bus.mem_read(self.registers.pc);
        let opcode = get_opcode(opcode_byte);
        let initial_pc = self.registers.pc;
        self.registers.pc = self.registers.pc.wrapping_add(1);

        let mut additional_cycles: u8 = 0;

        let (operand_addr, page_cross) = self.get_operand_address(opcode.mode, bus);

        match opcode.mnemonic {
            // Load / Store Operations
            "LDA" => {
                self.registers.a = bus.mem_read(operand_addr);
                self.registers.update_zero_negative_flags(self.registers.a);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "LDX" => {
                self.registers.x = bus.mem_read(operand_addr);
                self.registers.update_zero_negative_flags(self.registers.x);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "LDY" => {
                self.registers.y = bus.mem_read(operand_addr);
                self.registers.update_zero_negative_flags(self.registers.y);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "STA" => {
                let write_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, self.registers.a, write_cycle);
            }
            "STX" => {
                let write_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, self.registers.x, write_cycle);
            }
            "STY" => {
                let write_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, self.registers.y, write_cycle);
            }

            // Register Transfers
            "TAX" => {
                self.registers.x = self.registers.a;
                self.registers.update_zero_negative_flags(self.registers.x);
            }
            "TAY" => {
                self.registers.y = self.registers.a;
                self.registers.update_zero_negative_flags(self.registers.y);
            }
            "TXA" => {
                self.registers.a = self.registers.x;
                self.registers.update_zero_negative_flags(self.registers.a);
            }
            "TYA" => {
                self.registers.a = self.registers.y;
                self.registers.update_zero_negative_flags(self.registers.a);
            }
            "TSX" => {
                self.registers.x = self.registers.sp;
                self.registers.update_zero_negative_flags(self.registers.x);
            }
            "TXS" => {
                self.registers.sp = self.registers.x;
            }

            // Stack Operations
            "PHA" => {
                self.stack_push(bus, self.registers.a);
            }
            "PHP" => {
                let p = self.registers.p | CpuFlags::BREAK | CpuFlags::UNUSED;
                self.stack_push(bus, p);
            }
            "PLA" => {
                self.registers.a = self.stack_pop(bus);
                self.registers.update_zero_negative_flags(self.registers.a);
            }
            "PLP" => {
                let pulled = self.stack_pop(bus);
                self.registers.p = (pulled & !CpuFlags::BREAK) | CpuFlags::UNUSED;
            }

            // Logical Operations
            "AND" => {
                self.registers.a &= bus.mem_read(operand_addr);
                self.registers.update_zero_negative_flags(self.registers.a);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "EOR" => {
                self.registers.a ^= bus.mem_read(operand_addr);
                self.registers.update_zero_negative_flags(self.registers.a);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "ORA" => {
                self.registers.a |= bus.mem_read(operand_addr);
                self.registers.update_zero_negative_flags(self.registers.a);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "BIT" => {
                let val = bus.mem_read(operand_addr);
                self.registers
                    .set_flag(CpuFlags::ZERO, (self.registers.a & val) == 0);
                self.registers
                    .set_flag(CpuFlags::NEGATIVE, (val & 0x80) != 0);
                self.registers
                    .set_flag(CpuFlags::OVERFLOW, (val & 0x40) != 0);
            }

            // Arithmetic Operations
            "ADC" => {
                self.adc(bus.mem_read(operand_addr));
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "SBC" => {
                self.sbc(bus.mem_read(operand_addr));
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "CMP" => {
                let val = bus.mem_read(operand_addr);
                self.compare(self.registers.a, val);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "CPX" => {
                let val = bus.mem_read(operand_addr);
                self.compare(self.registers.x, val);
            }
            "CPY" => {
                let val = bus.mem_read(operand_addr);
                self.compare(self.registers.y, val);
            }

            // Increments & Decrements
            "INC" => {
                let orig = bus.mem_read(operand_addr);
                let val = orig.wrapping_add(1);
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.registers.update_zero_negative_flags(val);
            }
            "INX" => {
                self.registers.x = self.registers.x.wrapping_add(1);
                self.registers.update_zero_negative_flags(self.registers.x);
            }
            "INY" => {
                self.registers.y = self.registers.y.wrapping_add(1);
                self.registers.update_zero_negative_flags(self.registers.y);
            }
            "DEC" => {
                let orig = bus.mem_read(operand_addr);
                let val = orig.wrapping_sub(1);
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.registers.update_zero_negative_flags(val);
            }
            "DEX" => {
                self.registers.x = self.registers.x.wrapping_sub(1);
                self.registers.update_zero_negative_flags(self.registers.x);
            }
            "DEY" => {
                self.registers.y = self.registers.y.wrapping_sub(1);
                self.registers.update_zero_negative_flags(self.registers.y);
            }

            // Shifts
            "ASL" => {
                if opcode.mode == AddressingMode::Accumulator {
                    self.registers
                        .set_flag(CpuFlags::CARRY, (self.registers.a & 0x80) != 0);
                    self.registers.a <<= 1;
                    self.registers.update_zero_negative_flags(self.registers.a);
                } else {
                    let orig = bus.mem_read(operand_addr);
                    self.registers.set_flag(CpuFlags::CARRY, (orig & 0x80) != 0);
                    let val = orig << 1;
                    let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                    let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                    bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                    bus.mem_write_cycle(operand_addr, val, final_cycle);
                    self.registers.update_zero_negative_flags(val);
                }
            }
            "LSR" => {
                if opcode.mode == AddressingMode::Accumulator {
                    self.registers
                        .set_flag(CpuFlags::CARRY, (self.registers.a & 0x01) != 0);
                    self.registers.a >>= 1;
                    self.registers.update_zero_negative_flags(self.registers.a);
                } else {
                    let orig = bus.mem_read(operand_addr);
                    self.registers.set_flag(CpuFlags::CARRY, (orig & 0x01) != 0);
                    let val = orig >> 1;
                    let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                    let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                    bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                    bus.mem_write_cycle(operand_addr, val, final_cycle);
                    self.registers.update_zero_negative_flags(val);
                }
            }
            "ROL" => {
                let old_carry = if self.registers.get_flag(CpuFlags::CARRY) {
                    1
                } else {
                    0
                };
                if opcode.mode == AddressingMode::Accumulator {
                    self.registers
                        .set_flag(CpuFlags::CARRY, (self.registers.a & 0x80) != 0);
                    self.registers.a = (self.registers.a << 1) | old_carry;
                    self.registers.update_zero_negative_flags(self.registers.a);
                } else {
                    let orig = bus.mem_read(operand_addr);
                    self.registers.set_flag(CpuFlags::CARRY, (orig & 0x80) != 0);
                    let val = (orig << 1) | old_carry;
                    let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                    let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                    bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                    bus.mem_write_cycle(operand_addr, val, final_cycle);
                    self.registers.update_zero_negative_flags(val);
                }
            }
            "ROR" => {
                let old_carry = if self.registers.get_flag(CpuFlags::CARRY) {
                    0x80
                } else {
                    0
                };
                if opcode.mode == AddressingMode::Accumulator {
                    self.registers
                        .set_flag(CpuFlags::CARRY, (self.registers.a & 0x01) != 0);
                    self.registers.a = (self.registers.a >> 1) | old_carry;
                    self.registers.update_zero_negative_flags(self.registers.a);
                } else {
                    let orig = bus.mem_read(operand_addr);
                    self.registers.set_flag(CpuFlags::CARRY, (orig & 0x01) != 0);
                    let val = (orig >> 1) | old_carry;
                    let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                    let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                    bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                    bus.mem_write_cycle(operand_addr, val, final_cycle);
                    self.registers.update_zero_negative_flags(val);
                }
            }

            // Jumps & Calls
            "JMP" => {
                self.registers.pc = operand_addr;
            }
            "JSR" => {
                self.stack_push_u16(bus, self.registers.pc.wrapping_sub(1));
                self.registers.pc = operand_addr;
            }
            "RTS" => {
                self.registers.pc = self.stack_pop_u16(bus).wrapping_add(1);
            }
            "RTI" => {
                let pulled = self.stack_pop(bus);
                self.registers.p = (pulled & !CpuFlags::BREAK) | CpuFlags::UNUSED;
                self.registers.pc = self.stack_pop_u16(bus);
            }

            // Branches
            "BCC" => self.branch(
                !self.registers.get_flag(CpuFlags::CARRY),
                operand_addr,
                &mut additional_cycles,
            ),
            "BCS" => self.branch(
                self.registers.get_flag(CpuFlags::CARRY),
                operand_addr,
                &mut additional_cycles,
            ),
            "BEQ" => self.branch(
                self.registers.get_flag(CpuFlags::ZERO),
                operand_addr,
                &mut additional_cycles,
            ),
            "BMI" => self.branch(
                self.registers.get_flag(CpuFlags::NEGATIVE),
                operand_addr,
                &mut additional_cycles,
            ),
            "BNE" => self.branch(
                !self.registers.get_flag(CpuFlags::ZERO),
                operand_addr,
                &mut additional_cycles,
            ),
            "BPL" => self.branch(
                !self.registers.get_flag(CpuFlags::NEGATIVE),
                operand_addr,
                &mut additional_cycles,
            ),
            "BVC" => self.branch(
                !self.registers.get_flag(CpuFlags::OVERFLOW),
                operand_addr,
                &mut additional_cycles,
            ),
            "BVS" => self.branch(
                self.registers.get_flag(CpuFlags::OVERFLOW),
                operand_addr,
                &mut additional_cycles,
            ),

            // Status Flag Changes
            "CLC" => self.registers.set_flag(CpuFlags::CARRY, false),
            "CLD" => self.registers.set_flag(CpuFlags::DECIMAL, false),
            "CLI" => self.registers.set_flag(CpuFlags::INTERRUPT_DISABLE, false),
            "CLV" => self.registers.set_flag(CpuFlags::OVERFLOW, false),
            "SEC" => self.registers.set_flag(CpuFlags::CARRY, true),
            "SED" => self.registers.set_flag(CpuFlags::DECIMAL, true),
            "SEI" => self.registers.set_flag(CpuFlags::INTERRUPT_DISABLE, true),

            // System Functions
            "BRK" => {
                self.registers.pc = self.registers.pc.wrapping_add(1);
                self.stack_push_u16(bus, self.registers.pc);
                let p = self.registers.p | CpuFlags::BREAK | CpuFlags::UNUSED;
                self.stack_push(bus, p);
                self.registers.set_flag(CpuFlags::INTERRUPT_DISABLE, true);
                self.registers.pc = bus.mem_read_u16(0xFFFE);
            }
            "NOP" => {
                if page_cross {
                    additional_cycles += 1;
                }
            }

            // Unofficial Opcodes
            "LAX" => {
                let val = bus.mem_read(operand_addr);
                self.registers.a = val;
                self.registers.x = val;
                self.registers.update_zero_negative_flags(val);
                if page_cross {
                    additional_cycles += 1;
                }
            }
            "SAX" => {
                let val = self.registers.a & self.registers.x;
                let write_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, val, write_cycle);
            }
            "DCP" => {
                let orig = bus.mem_read(operand_addr);
                let val = orig.wrapping_sub(1);
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.compare(self.registers.a, val);
            }
            "ISC" => {
                let orig = bus.mem_read(operand_addr);
                let val = orig.wrapping_add(1);
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.sbc(val);
            }
            "SLO" => {
                let orig = bus.mem_read(operand_addr);
                self.registers.set_flag(CpuFlags::CARRY, (orig & 0x80) != 0);
                let val = orig << 1;
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.registers.a |= val;
                self.registers.update_zero_negative_flags(self.registers.a);
            }
            "RLA" => {
                let old_carry = if self.registers.get_flag(CpuFlags::CARRY) {
                    1
                } else {
                    0
                };
                let orig = bus.mem_read(operand_addr);
                self.registers.set_flag(CpuFlags::CARRY, (orig & 0x80) != 0);
                let val = (orig << 1) | old_carry;
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.registers.a &= val;
                self.registers.update_zero_negative_flags(self.registers.a);
            }
            "SRE" => {
                let orig = bus.mem_read(operand_addr);
                self.registers.set_flag(CpuFlags::CARRY, (orig & 0x01) != 0);
                let val = orig >> 1;
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.registers.a ^= val;
                self.registers.update_zero_negative_flags(self.registers.a);
            }
            "RRA" => {
                let old_carry = if self.registers.get_flag(CpuFlags::CARRY) {
                    0x80
                } else {
                    0
                };
                let orig = bus.mem_read(operand_addr);
                self.registers.set_flag(CpuFlags::CARRY, (orig & 0x01) != 0);
                let val = (orig >> 1) | old_carry;
                let dummy_cycle = self.total_cycles + opcode.cycles as u64 - 2;
                let final_cycle = self.total_cycles + opcode.cycles as u64 - 1;
                bus.mem_write_cycle(operand_addr, orig, dummy_cycle);
                bus.mem_write_cycle(operand_addr, val, final_cycle);
                self.adc(val);
            }
            "ANC" => {
                self.registers.a &= bus.mem_read(operand_addr);
                self.registers.update_zero_negative_flags(self.registers.a);
                self.registers
                    .set_flag(CpuFlags::CARRY, self.registers.get_flag(CpuFlags::NEGATIVE));
            }
            "ALR" => {
                self.registers.a &= bus.mem_read(operand_addr);
                self.registers
                    .set_flag(CpuFlags::CARRY, (self.registers.a & 0x01) != 0);
                self.registers.a >>= 1;
                self.registers.update_zero_negative_flags(self.registers.a);
            }
            "ARR" => {
                self.registers.a &= bus.mem_read(operand_addr);
                let old_carry = if self.registers.get_flag(CpuFlags::CARRY) {
                    0x80
                } else {
                    0
                };
                self.registers.a = (self.registers.a >> 1) | old_carry;
                self.registers.update_zero_negative_flags(self.registers.a);
                let bit6 = (self.registers.a >> 6) & 1;
                let bit5 = (self.registers.a >> 5) & 1;
                self.registers.set_flag(CpuFlags::CARRY, bit6 != 0);
                self.registers
                    .set_flag(CpuFlags::OVERFLOW, (bit6 ^ bit5) != 0);
            }
            "AXS" => {
                let temp = (self.registers.a & self.registers.x) as u16;
                let val = bus.mem_read(operand_addr) as u16;
                let diff = temp.wrapping_sub(val);
                self.registers.set_flag(CpuFlags::CARRY, temp >= val);
                self.registers.x = (diff & 0xFF) as u8;
                self.registers.update_zero_negative_flags(self.registers.x);
            }
            "AHX" | "SHY" | "SHX" | "TAS" | "LAS" | "XAA" | "KIL" => {
                // Highly unstable/rare unofficial opcodes, handled gracefully
            }
            _ => panic!(
                "Unimplemented opcode {:02X}: {}",
                opcode.code, opcode.mnemonic
            ),
        }

        let cycles = opcode.cycles + additional_cycles;
        self.total_cycles += cycles as u64;
        let _ = initial_pc; // Suppress unused warning if any
        cycles
    }

    fn branch(&mut self, condition: bool, target_addr: u16, additional_cycles: &mut u8) {
        if condition {
            *additional_cycles += 1;
            if (self.registers.pc & 0xFF00) != (target_addr & 0xFF00) {
                *additional_cycles += 1;
            }
            self.registers.pc = target_addr;
        }
    }

    fn compare(&mut self, reg: u8, val: u8) {
        self.registers.set_flag(CpuFlags::CARRY, reg >= val);
        self.registers
            .update_zero_negative_flags(reg.wrapping_sub(val));
    }

    fn adc(&mut self, val: u8) {
        let a = self.registers.a as u16;
        let b = val as u16;
        let c = if self.registers.get_flag(CpuFlags::CARRY) {
            1
        } else {
            0
        };
        let sum = a + b + c;

        self.registers.set_flag(CpuFlags::CARRY, sum > 0xFF);
        let result = (sum & 0xFF) as u8;
        self.registers.set_flag(
            CpuFlags::OVERFLOW,
            (!(self.registers.a ^ val) & (self.registers.a ^ result) & 0x80) != 0,
        );
        self.registers.a = result;
        self.registers.update_zero_negative_flags(self.registers.a);
    }

    fn sbc(&mut self, val: u8) {
        // SBC val is identical to ADC (val ^ 0xFF)
        self.adc(val ^ 0xFF);
    }

    pub fn get_operand_address(
        &mut self,
        mode: AddressingMode,
        bus: &mut impl Memory,
    ) -> (u16, bool) {
        match mode {
            AddressingMode::Implied | AddressingMode::Accumulator => (0, false),
            AddressingMode::Immediate => {
                let addr = self.registers.pc;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                (addr, false)
            }
            AddressingMode::ZeroPage => {
                let addr = bus.mem_read(self.registers.pc) as u16;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                (addr, false)
            }
            AddressingMode::ZeroPageX => {
                let base = bus.mem_read(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let addr = base.wrapping_add(self.registers.x) as u16;
                (addr, false)
            }
            AddressingMode::ZeroPageY => {
                let base = bus.mem_read(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let addr = base.wrapping_add(self.registers.y) as u16;
                (addr, false)
            }
            AddressingMode::Relative => {
                let offset = bus.mem_read(self.registers.pc) as i8 as i16;
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let target = self.registers.pc.wrapping_add(offset as u16);
                (target, false)
            }
            AddressingMode::Absolute => {
                let addr = bus.mem_read_u16(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(2);
                (addr, false)
            }
            AddressingMode::AbsoluteX => {
                let base = bus.mem_read_u16(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(2);
                let addr = base.wrapping_add(self.registers.x as u16);
                let page_cross = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_cross)
            }
            AddressingMode::AbsoluteY => {
                let base = bus.mem_read_u16(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(2);
                let addr = base.wrapping_add(self.registers.y as u16);
                let page_cross = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_cross)
            }
            AddressingMode::Indirect => {
                let ptr = bus.mem_read_u16(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(2);
                // 6502 page wrap bug
                let lo = bus.mem_read(ptr) as u16;
                let hi = bus.mem_read((ptr & 0xFF00) | ((ptr + 1) & 0x00FF)) as u16;
                ((hi << 8) | lo, false)
            }
            AddressingMode::IndexedIndirect => {
                let base = bus.mem_read(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let ptr = base.wrapping_add(self.registers.x);
                let lo = bus.mem_read(ptr as u16) as u16;
                let hi = bus.mem_read(ptr.wrapping_add(1) as u16) as u16;
                ((hi << 8) | lo, false)
            }
            AddressingMode::IndirectIndexed => {
                let base = bus.mem_read(self.registers.pc);
                self.registers.pc = self.registers.pc.wrapping_add(1);
                let lo = bus.mem_read(base as u16) as u16;
                let hi = bus.mem_read(base.wrapping_add(1) as u16) as u16;
                let deref_base = (hi << 8) | lo;
                let addr = deref_base.wrapping_add(self.registers.y as u16);
                let page_cross = (deref_base & 0xFF00) != (addr & 0xFF00);
                (addr, page_cross)
            }
        }
    }
}
