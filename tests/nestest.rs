use nes::bus::Bus;
use nes::cartridge::Cartridge;
use nes::cpu::Cpu;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[test]
fn test_nestest_execution() {
    let rom_path = std::env::var("NESTEST_ROM")
        .unwrap_or_else(|_| "tests/fixtures/nestest.nes".to_string());
    let rom_bytes =
        std::fs::read(&rom_path).unwrap_or_else(|e| panic!("Failed to read nestest fixture from {}: {}", rom_path, e));
    let cartridge = Cartridge::from_bytes(&rom_bytes).expect("Failed to parse nestest cartridge");

    let mut bus = Bus::new(cartridge);
    let mut cpu = Cpu::new();

    // nestest automation start conditions
    cpu.reset(&mut bus);
    cpu.registers.pc = 0xC000;
    cpu.registers.p = 0x24;
    cpu.registers.sp = 0xFD;
    cpu.total_cycles = 7;

    let log_file =
        File::open("tests/fixtures/nestest.log").expect("Failed to open nestest.log fixture");
    let reader = BufReader::new(log_file);

    let mut line_num = 0;
    for line_result in reader.lines() {
        let expected_line = line_result.expect("Failed to read log line");
        let expected_line = expected_line.trim_end();
        if expected_line.is_empty() {
            continue;
        }
        line_num += 1;

        // Parse expected fields from nestest.log:
        // Example: C000  4C F5 C5  JMP $C5F5                       A:00 X:00 Y:00 P:24 SP:FD PPU:  0, 21 CYC:7
        let exp_pc = u16::from_str_radix(&expected_line[0..4], 16).unwrap();

        let a_idx = expected_line.find("A:").expect("find A:");
        let exp_a = u8::from_str_radix(&expected_line[a_idx + 2..a_idx + 4], 16).expect("parse A");

        let x_idx = expected_line.find("X:").expect("find X:");
        let exp_x = u8::from_str_radix(&expected_line[x_idx + 2..x_idx + 4], 16).expect("parse X");

        let y_idx = expected_line.find("Y:").expect("find Y:");
        let exp_y = u8::from_str_radix(&expected_line[y_idx + 2..y_idx + 4], 16).expect("parse Y");

        let p_idx = expected_line.find("P:").expect("find P:");
        let exp_p = u8::from_str_radix(&expected_line[p_idx + 2..p_idx + 4], 16).expect("parse P");

        let sp_idx = expected_line.find("SP:").expect("find SP:");
        let exp_sp =
            u8::from_str_radix(&expected_line[sp_idx + 3..sp_idx + 5], 16).expect("parse SP");

        let cyc_idx = expected_line.find("CYC:").expect("find CYC:");
        let exp_cyc: u64 = expected_line[cyc_idx + 4..]
            .trim()
            .parse()
            .expect("parse CYC");

        assert_eq!(
            cpu.registers.pc, exp_pc,
            "Line {}: PC mismatch! Expected {:04X}, got {:04X}\nLog: {}",
            line_num, exp_pc, cpu.registers.pc, expected_line
        );
        assert_eq!(
            cpu.registers.a, exp_a,
            "Line {}: A mismatch! Expected {:02X}, got {:02X}\nLog: {}",
            line_num, exp_a, cpu.registers.a, expected_line
        );
        assert_eq!(
            cpu.registers.x, exp_x,
            "Line {}: X mismatch! Expected {:02X}, got {:02X}\nLog: {}",
            line_num, exp_x, cpu.registers.x, expected_line
        );
        assert_eq!(
            cpu.registers.y, exp_y,
            "Line {}: Y mismatch! Expected {:02X}, got {:02X}\nLog: {}",
            line_num, exp_y, cpu.registers.y, expected_line
        );
        assert_eq!(
            cpu.registers.p, exp_p,
            "Line {}: P mismatch! Expected {:02X}, got {:02X}\nLog: {}",
            line_num, exp_p, cpu.registers.p, expected_line
        );
        assert_eq!(
            cpu.registers.sp, exp_sp,
            "Line {}: SP mismatch! Expected {:02X}, got {:02X}\nLog: {}",
            line_num, exp_sp, cpu.registers.sp, expected_line
        );
        assert_eq!(
            cpu.total_cycles, exp_cyc,
            "Line {}: CYC mismatch! Expected {}, got {}\nLog: {}",
            line_num, exp_cyc, cpu.total_cycles, expected_line
        );

        cpu.step(&mut bus);
    }

    println!(
        "All {} instructions in nestest.log matched perfectly!",
        line_num
    );
}
