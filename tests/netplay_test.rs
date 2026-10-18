use nes::{
    Cartridge, InviteHash, JoypadButton, LockstepPoll, Nes, NetplaySession, INPUT_FLAG_PAUSE,
    INPUT_FLAG_RESET,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

/// Builds a synthetic 2-player NES ROM whose 6502 program continuously strobes and reads
/// all 8 buttons from both Controller 1 ($4016) and Controller 2 ($4017) and mixes them
/// into zero-page RAM ($0010..$0013). Any difference in controller inputs or frame timing
/// immediately causes CPU RAM and `state_checksum()` to diverge.
fn create_two_player_test_rom(seed: u8) -> Cartridge {
    let mut rom = vec![0u8; 16 + 16384 + 8192];
    rom[0..4].copy_from_slice(b"NES\x1A");
    rom[4] = 1; // 16KB PRG ROM
    rom[5] = 1; // 8KB CHR ROM
    rom[6] = 0x00;

    // 6502 program at $8000 (offset 16 in file):
    // Loop:
    //   LDA #$01
    //   STA $4016
    //   LDA #$00
    //   STA $4016
    //   LDX #$08
    // ReadLoop:
    //   LDA $4016
    //   AND #$01
    //   ASL $10
    //   ORA $10
    //   STA $10
    //   LDA $4017
    //   AND #$01
    //   ASL $11
    //   ORA $11
    //   STA $11
    //   DEX
    //   BNE ReadLoop
    //   LDA $12
    //   CLC
    //   ADC $10
    //   EOR $11
    //   STA $12
    //   INC $13
    //   JMP $8000
    let prg_code: &[u8] = &[
        0xA9, 0x01, // LDA #$01
        0x8D, 0x16, 0x40, // STA $4016
        0xA9, 0x00, // LDA #$00
        0x8D, 0x16, 0x40, // STA $4016
        0xA2, 0x08, // LDX #$08
        // ReadLoop ($800C):
        0xAD, 0x16, 0x40, // LDA $4016
        0x29, 0x01, // AND #$01
        0x06, 0x10, // ASL $10
        0x05, 0x10, // ORA $10
        0x85, 0x10, // STA $10
        0xAD, 0x17, 0x40, // LDA $4017
        0x29, 0x01, // AND #$01
        0x06, 0x11, // ASL $11
        0x05, 0x11, // ORA $11
        0x85, 0x11, // STA $11
        0xCA, // DEX
        0xD0, 0xE6, // BNE ReadLoop (-26)
        0xA5, 0x12, // LDA $12
        0x18, // CLC
        0x65, 0x10, // ADC $10
        0x45, 0x11, // EOR $11
        0x85, 0x12, // STA $12
        0xE6, 0x13, // INC $13
        0x4C, 0x00, 0x80, // JMP $8000
    ];

    rom[16..16 + prg_code.len()].copy_from_slice(prg_code);
    rom[16 + 0x100] = seed;

    // Reset vector at $FFFC -> $8000 (offset 16 + 0x3FFC in 16KB NROM)
    rom[16 + 0x3FFC] = 0x00;
    rom[16 + 0x3FFD] = 0x80;

    Cartridge::from_bytes(&rom).expect("valid test ROM")
}

#[test]
fn test_invite_hash_ipv4_encode_decode_roundtrip() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)), 7878);
    let original = InviteHash::new(addr, 0xDEADBEEF, 0x4A7F, 3);
    let encoded = original.encode();

    assert!(encoded.starts_with("NES-"));
    // 24 Base32 chars + "NES-" (4) + 3 hyphens = 31 chars total
    assert_eq!(encoded.len(), 31);

    let decoded = InviteHash::decode(&encoded).expect("should decode valid hash");
    assert_eq!(decoded, original);

    // Case-insensitive and without dashes or prefix
    let lower_no_dashes = encoded.to_lowercase().replace("nes-", "").replace('-', "");
    let decoded2 = InviteHash::decode(&lower_no_dashes).expect("should decode stripped hash");
    assert_eq!(decoded2, original);
}

#[test]
fn test_invite_hash_ipv6_encode_decode_roundtrip() {
    let ipv6 = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1);
    let addr = SocketAddr::new(IpAddr::V6(ipv6), 9999);
    let original = InviteHash::new(addr, 0x12345678, 0xBEEF, 2);
    let encoded = original.encode();

    assert!(encoded.starts_with("NES-"));
    let decoded = InviteHash::decode(&encoded).expect("should decode IPv6 hash");
    assert_eq!(decoded, original);
}

#[test]
fn test_invite_hash_detects_typos_and_tampering() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 15)), 7878);
    let original = InviteHash::new(addr, 0xCAFEBABE, 0x1337, 2);
    let encoded = original.encode();

    // Flip one character in the middle of the hash
    let mut chars: Vec<char> = encoded.chars().collect();
    chars[10] = if chars[10] == 'A' { 'B' } else { 'A' };
    let corrupted: String = chars.into_iter().collect();

    let err = InviteHash::decode(&corrupted).expect_err("corrupted hash must fail CRC-8 check");
    assert!(err.contains("checksum"));
}

#[test]
fn test_p2p_udp_lockstep_synchronization_and_controls() {
    let host_cart = create_two_player_test_rom(0x42);
    let peer_cart = create_two_player_test_rom(0x42);
    assert_eq!(host_cart.rom_hash, peer_cart.rom_hash);

    let mut host_nes = Nes::new(host_cart);
    let mut peer_nes = Nes::new(peer_cart);

    let mut host_session = NetplaySession::new_host(
        0, // Ephemeral port
        Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        host_nes.rom_hash(),
        2,
        false,
    )
    .expect("host session should bind");

    let custom_hash = host_session.invite_hash.encode();

    let mut peer_session = NetplaySession::new_peer(&custom_hash, peer_nes.rom_hash(), false)
        .expect("peer session should parse custom hash and bind");

    let mut host_frames = 0u32;
    let mut peer_frames = 0u32;
    let target_frames = 80u32;
    let start = Instant::now();

    while host_frames < target_frames || peer_frames < target_frames {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "P2P lockstep timed out (host_frames={}, peer_frames={})",
            host_frames,
            peer_frames
        );

        // Simulate dynamic Player 1 and Player 2 inputs across frames
        let p1_local_btn = match host_frames % 5 {
            0 => 1 << (JoypadButton::A as u8),
            1 => 1 << (JoypadButton::Right as u8),
            2 => (1 << (JoypadButton::B as u8)) | (1 << (JoypadButton::Up as u8)),
            3 => 1 << (JoypadButton::Start as u8),
            _ => 0,
        };

        let p2_local_btn = match peer_frames % 4 {
            0 => 1 << (JoypadButton::B as u8),
            1 => 1 << (JoypadButton::Left as u8),
            2 => (1 << (JoypadButton::A as u8)) | (1 << (JoypadButton::Down as u8)),
            _ => 0,
        };

        if host_frames < target_frames {
            if let LockstepPoll::FrameReady(sync) =
                host_session.poll_step(p1_local_btn, 0, host_nes.state_checksum())
            {
                assert_eq!(sync.frame, host_frames);
                host_nes.set_buttons_p1(sync.p1_buttons);
                host_nes.set_buttons_p2(sync.p2_buttons);
                host_nes.step_frame();
                host_frames += 1;
            }
        }

        if peer_frames < target_frames {
            if let LockstepPoll::FrameReady(sync) =
                peer_session.poll_step(p2_local_btn, 0, peer_nes.state_checksum())
            {
                assert_eq!(sync.frame, peer_frames);
                peer_nes.set_buttons_p1(sync.p1_buttons);
                peer_nes.set_buttons_p2(sync.p2_buttons);
                peer_nes.step_frame();
                peer_frames += 1;
            }
        }

        std::thread::sleep(Duration::from_micros(200));
    }

    // Both emulators must have identical state checksums, CPU RAM, and zero desyncs!
    assert_eq!(host_session.desync_count, 0);
    assert_eq!(peer_session.desync_count, 0);
    assert_eq!(host_nes.state_checksum(), peer_nes.state_checksum());
    assert_eq!(host_nes.bus.cpu_ram, peer_nes.bus.cpu_ram);
}

#[test]
fn test_p2p_synchronized_reset_and_pause() {
    let host_cart = create_two_player_test_rom(0x99);
    let peer_cart = create_two_player_test_rom(0x99);

    let mut host_nes = Nes::new(host_cart);
    let mut peer_nes = Nes::new(peer_cart);

    let mut host_session = NetplaySession::new_host(
        0,
        Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        host_nes.rom_hash(),
        2,
        false,
    )
    .unwrap();

    let hash_str = host_session.invite_hash.encode();
    let mut peer_session = NetplaySession::new_peer(&hash_str, peer_nes.rom_hash(), false).unwrap();

    let mut host_frames = 0u32;
    let mut peer_frames = 0u32;
    let mut host_reset_frame = None;
    let mut peer_reset_frame = None;
    let mut host_pause_frame = None;
    let mut peer_pause_frame = None;

    let start = Instant::now();
    while host_frames < 40 || peer_frames < 40 {
        assert!(start.elapsed() < Duration::from_secs(5));

        if host_frames == 10 && host_reset_frame.is_none() {
            host_session.queue_local_flag(INPUT_FLAG_RESET);
        }
        if peer_frames == 20 && peer_pause_frame.is_none() {
            peer_session.queue_local_flag(INPUT_FLAG_PAUSE);
        }

        if host_frames < 40 {
            if let LockstepPoll::FrameReady(sync) =
                host_session.poll_step(0x03, 0, host_nes.state_checksum())
            {
                if sync.reset_triggered {
                    host_reset_frame = Some(sync.frame);
                    host_nes.reset();
                }
                if sync.pause_toggled {
                    host_pause_frame = Some(sync.frame);
                }
                host_nes.set_buttons_p1(sync.p1_buttons);
                host_nes.set_buttons_p2(sync.p2_buttons);
                host_nes.step_frame();
                host_frames += 1;
            }
        }

        if peer_frames < 40 {
            if let LockstepPoll::FrameReady(sync) =
                peer_session.poll_step(0x0C, 0, peer_nes.state_checksum())
            {
                if sync.reset_triggered {
                    peer_reset_frame = Some(sync.frame);
                    peer_nes.reset();
                }
                if sync.pause_toggled {
                    peer_pause_frame = Some(sync.frame);
                }
                peer_nes.set_buttons_p1(sync.p1_buttons);
                peer_nes.set_buttons_p2(sync.p2_buttons);
                peer_nes.step_frame();
                peer_frames += 1;
            }
        }

        std::thread::sleep(Duration::from_micros(200));
    }

    assert!(host_reset_frame.is_some());
    assert_eq!(host_reset_frame, peer_reset_frame);
    assert!(host_pause_frame.is_some());
    assert_eq!(host_pause_frame, peer_pause_frame);
    assert_eq!(host_nes.state_checksum(), peer_nes.state_checksum());
}

#[test]
fn test_p2p_rom_mismatch_rejection() {
    let host_cart = create_two_player_test_rom(0x01);
    let different_cart = create_two_player_test_rom(0x02);
    assert_ne!(host_cart.rom_hash, different_cart.rom_hash);

    let host_session = NetplaySession::new_host(
        0,
        Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        host_cart.rom_hash,
        2,
        false,
    )
    .unwrap();

    let hash_str = host_session.invite_hash.encode();
    let join_err = NetplaySession::new_peer(&hash_str, different_cart.rom_hash, false)
        .err()
        .expect("joining with a mismatched ROM must return an error");
    assert!(join_err.contains("ROM checksum mismatch"));
}
