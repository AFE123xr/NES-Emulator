use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 4] = b"NESP";
const VERSION_IPV4: u8 = 0x14;
const VERSION_IPV6: u8 = 0x16;

const PKT_HANDSHAKE_REQ: u8 = 0x01;
const PKT_HANDSHAKE_ACK: u8 = 0x02;
const PKT_HANDSHAKE_REJECT: u8 = 0x03;
const PKT_FRAME_INPUT: u8 = 0x04;
const PKT_PING: u8 = 0x05;
const PKT_PONG: u8 = 0x06;
const PKT_DISCONNECT: u8 = 0x07;

const REJECT_ROM_MISMATCH: u8 = 0x01;
const REJECT_INVALID_TOKEN: u8 = 0x02;
const REJECT_SESSION_FULL: u8 = 0x03;

pub const INPUT_FLAG_RESET: u8 = 0x01;
pub const INPUT_FLAG_PAUSE: u8 = 0x02;

const MAX_INPUT_HISTORY: usize = 16;
pub const DEFAULT_NETPLAY_PORT: u16 = 7878;
pub const DEFAULT_INPUT_DELAY: u8 = 2;

/// Crockford's Base32 alphabet (avoids I, L, O, U to prevent visual ambiguity).
const BASE32_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Role of the local emulator instance in a P2P session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetplayRole {
    /// Host is Player 1 and listens for an incoming peer connection.
    Host,
    /// Peer is Player 2 and connects to the Host using a shareable `InviteHash`.
    Peer,
}

/// Configuration for initializing a P2P multiplayer session.
#[derive(Debug, Clone)]
pub struct NetplayConfig {
    pub role: NetplayRole,
    pub port: u16,
    pub host_ip_override: Option<IpAddr>,
    pub join_hash: Option<String>,
    pub input_delay: u8,
    pub ignore_rom_hash: bool,
}

impl Default for NetplayConfig {
    fn default() -> Self {
        Self {
            role: NetplayRole::Host,
            port: DEFAULT_NETPLAY_PORT,
            host_ip_override: None,
            join_hash: None,
            input_delay: DEFAULT_INPUT_DELAY,
            ignore_rom_hash: false,
        }
    }
}

/// Compact, human-shareable custom hash encoding the P2P connection parameters:
/// - Host IP address (IPv4 or IPv6)
/// - Host UDP port (`u16`)
/// - Cartridge ROM 32-bit FNV-1a checksum (`u32`)
/// - 16-bit Session Token (`u16`)
/// - Lockstep frame input delay (`u8`)
/// - 8-bit CRC-8 integrity checksum (`u8`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteHash {
    pub addr: SocketAddr,
    pub rom_hash: u32,
    pub session_token: u16,
    pub input_delay: u8,
}

impl InviteHash {
    pub fn new(addr: SocketAddr, rom_hash: u32, session_token: u16, input_delay: u8) -> Self {
        Self {
            addr,
            rom_hash,
            session_token,
            input_delay: input_delay.clamp(1, 10),
        }
    }

    /// Encodes the session details into a shareable hash string, e.g.
    /// `NES-7K4M9P-2X8QR5-T1W9B3-N6C4H8`.
    pub fn encode(&self) -> String {
        match self.addr.ip() {
            IpAddr::V4(ipv4) => {
                // 15 bytes = 120 bits = exactly 24 Base32 characters
                let mut buf = [0u8; 15];
                buf[0] = VERSION_IPV4;
                buf[1..5].copy_from_slice(&ipv4.octets());
                buf[5..7].copy_from_slice(&self.addr.port().to_be_bytes());
                buf[7..11].copy_from_slice(&self.rom_hash.to_be_bytes());
                buf[11..13].copy_from_slice(&self.session_token.to_be_bytes());
                buf[13] = self.input_delay;
                let crc = crc8(&buf[0..14]);
                buf[14] = crc;
                xor_transform(&mut buf[0..14], crc);
                let raw = base32_encode(&buf);
                format_hash_groups(&raw)
            }
            IpAddr::V6(ipv6) => {
                // 30 bytes = 240 bits = exactly 48 Base32 characters
                let mut buf = [0u8; 30];
                buf[0] = VERSION_IPV6;
                buf[1..17].copy_from_slice(&ipv6.octets());
                buf[17..19].copy_from_slice(&self.addr.port().to_be_bytes());
                buf[19..23].copy_from_slice(&self.rom_hash.to_be_bytes());
                buf[23..25].copy_from_slice(&self.session_token.to_be_bytes());
                buf[25] = self.input_delay;
                buf[26] = 0;
                buf[27] = 0;
                buf[28] = 0;
                let crc = crc8(&buf[0..29]);
                buf[29] = crc;
                xor_transform(&mut buf[0..29], crc);
                let raw = base32_encode(&buf);
                format_hash_groups(&raw)
            }
        }
    }

    /// Decodes and validates a shareable hash string (case-insensitive, ignores `NES-` prefix and dashes).
    pub fn decode(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        let without_prefix = if trimmed.len() >= 4
            && (trimmed[..4].eq_ignore_ascii_case("NES-")
                || trimmed[..4].eq_ignore_ascii_case("NES:"))
        {
            &trimmed[4..]
        } else {
            trimmed
        };

        let cleaned: String = without_prefix
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
            .collect();

        if cleaned.is_empty() {
            return Err("Invite hash is empty".to_string());
        }

        let mut bytes = base32_decode(&cleaned)?;
        match bytes.len() {
            15 => {
                let crc = bytes[14];
                xor_transform(&mut bytes[0..14], crc);
                let expected_crc = crc8(&bytes[0..14]);
                if crc != expected_crc {
                    return Err(
                        "Invalid invite hash checksum — please check the code for typos"
                            .to_string(),
                    );
                }
                if bytes[0] != VERSION_IPV4 {
                    return Err(format!(
                        "Unsupported invite hash version: 0x{:02X}",
                        bytes[0]
                    ));
                }
                let ip = Ipv4Addr::new(bytes[1], bytes[2], bytes[3], bytes[4]);
                let port = u16::from_be_bytes([bytes[5], bytes[6]]);
                let rom_hash = u32::from_be_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]);
                let session_token = u16::from_be_bytes([bytes[11], bytes[12]]);
                let input_delay = bytes[13].clamp(1, 10);
                Ok(Self {
                    addr: SocketAddr::new(IpAddr::V4(ip), port),
                    rom_hash,
                    session_token,
                    input_delay,
                })
            }
            30 => {
                let crc = bytes[29];
                xor_transform(&mut bytes[0..29], crc);
                let expected_crc = crc8(&bytes[0..29]);
                if crc != expected_crc {
                    return Err(
                        "Invalid invite hash checksum — please check the code for typos"
                            .to_string(),
                    );
                }
                if bytes[0] != VERSION_IPV6 {
                    return Err(format!(
                        "Unsupported invite hash version: 0x{:02X}",
                        bytes[0]
                    ));
                }
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&bytes[1..17]);
                let ip = Ipv6Addr::from(octets);
                let port = u16::from_be_bytes([bytes[17], bytes[18]]);
                let rom_hash = u32::from_be_bytes([bytes[19], bytes[20], bytes[21], bytes[22]]);
                let session_token = u16::from_be_bytes([bytes[23], bytes[24]]);
                let input_delay = bytes[25].clamp(1, 10);
                Ok(Self {
                    addr: SocketAddr::new(IpAddr::V6(ip), port),
                    rom_hash,
                    session_token,
                    input_delay,
                })
            }
            other => Err(format!(
                "Invalid invite hash length (expected 24 or 48 characters, got {} characters / {} bytes)",
                cleaned.len(),
                other
            )),
        }
    }
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0x5A;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ 0x31;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

fn xor_transform(data: &mut [u8], seed: u8) {
    let mut state = (seed as u32).wrapping_mul(0x9E37_79B9) ^ 0xA55A_5AA5;
    for (i, byte) in data.iter_mut().enumerate() {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mask = ((state >> ((i % 4) * 8)) as u8) ^ seed.wrapping_add(i as u8);
        *byte ^= mask;
    }
}

fn base32_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer: u32 = 0;
    let mut bits_left: usize = 0;

    for &b in bytes {
        buffer = (buffer << 8) | (b as u32);
        bits_left += 8;
        while bits_left >= 5 {
            bits_left -= 5;
            let idx = ((buffer >> bits_left) & 0x1F) as usize;
            out.push(BASE32_ALPHABET[idx] as char);
        }
    }
    if bits_left > 0 {
        let idx = ((buffer << (5 - bits_left)) & 0x1F) as usize;
        out.push(BASE32_ALPHABET[idx] as char);
    }
    out
}

fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut buffer: u32 = 0;
    let mut bits_left: usize = 0;
    let mut out = Vec::with_capacity(s.len() * 5 / 8);

    for ch in s.chars() {
        let val: u8 = match ch.to_ascii_uppercase() {
            '0' | 'O' => 0,
            '1' | 'I' | 'L' => 1,
            '2' => 2,
            '3' => 3,
            '4' => 4,
            '5' => 5,
            '6' => 6,
            '7' => 7,
            '8' => 8,
            '9' => 9,
            'A' => 10,
            'B' => 11,
            'C' => 12,
            'D' => 13,
            'E' => 14,
            'F' => 15,
            'G' => 16,
            'H' => 17,
            'J' => 18,
            'K' => 19,
            'M' => 20,
            'N' => 21,
            'P' => 22,
            'Q' => 23,
            'R' => 24,
            'S' => 25,
            'T' => 26,
            'V' => 27,
            'W' => 28,
            'X' => 29,
            'Y' => 30,
            'Z' => 31,
            _ => return Err(format!("Invalid character '{}' in invite hash", ch)),
        };

        buffer = (buffer << 5) | (val as u32);
        bits_left += 5;
        if bits_left >= 8 {
            bits_left -= 8;
            out.push(((buffer >> bits_left) & 0xFF) as u8);
        }
    }

    Ok(out)
}

fn format_hash_groups(raw: &str) -> String {
    let mut result = String::from("NES");
    for (i, ch) in raw.chars().enumerate() {
        if i % 6 == 0 {
            result.push('-');
        }
        result.push(ch);
    }
    result
}

/// Auto-detects the local outbound LAN IP address (falls back to `127.0.0.1` if offline).
pub fn detect_local_ip() -> IpAddr {
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = sock.local_addr() {
                return local_addr.ip();
            }
        }
    }
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

/// Connection state of the P2P session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Host is waiting for Player 2 to connect, or Peer is handshaking with Host.
    Handshaking,
    /// Connected and executing frames in deterministic lockstep.
    Connected,
    /// Session ended due to peer disconnect or handshake rejection.
    Disconnected(String),
}

/// Synchronized inputs for a single frame returned by `NetplaySession::poll_frame`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynchronizedFrame {
    pub frame: u32,
    pub p1_buttons: u8,
    pub p2_buttons: u8,
    pub reset_triggered: bool,
    pub pause_toggled: bool,
}

/// Result of attempting to advance the lockstep session by one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockstepPoll {
    /// Still establishing handshake with remote peer.
    WaitingForHandshake,
    /// Waiting for remote peer's input for the current frame.
    WaitingForRemoteInput { frame: u32 },
    /// Both local and remote inputs for `frame` are available.
    FrameReady(SynchronizedFrame),
    /// Session was disconnected or rejected.
    Disconnected(String),
}

/// Manages a UDP-based deterministic lockstep P2P multiplayer session.
pub struct NetplaySession {
    socket: UdpSocket,
    pub role: NetplayRole,
    pub state: SessionState,
    pub invite_hash: InviteHash,
    pub localhost_hash: InviteHash,
    pub peer_addr: Option<SocketAddr>,
    pub rom_hash: u32,
    pub session_token: u16,
    pub input_delay: u8,
    pub ignore_rom_hash: bool,

    pub current_frame: u32,
    local_scheduled_up_to: Option<u32>,
    queued_local_flags: u8,
    local_inputs: HashMap<u32, (u8, u8)>,
    remote_inputs: HashMap<u32, (u8, u8)>,
    local_checksums: HashMap<u32, u32>,
    remote_checksums: HashMap<u32, u32>,
    pub desync_count: u32,

    start_instant: Instant,
    last_packet_sent: Instant,
    last_packet_received: Instant,
    last_ping_sent: Instant,
    pub rtt_ms: u32,
}

impl NetplaySession {
    /// Creates a new Host session (Player 1) bound to `0.0.0.0:port`.
    pub fn new_host(
        port: u16,
        host_ip_override: Option<IpAddr>,
        rom_hash: u32,
        input_delay: u8,
        ignore_rom_hash: bool,
    ) -> Result<Self, String> {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
        let socket = UdpSocket::bind(bind_addr)
            .map_err(|e| format!("Failed to bind UDP port {}: {}", port, e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking UDP socket: {}", e))?;

        let actual_port = socket
            .local_addr()
            .map_err(|e| format!("Failed to query local socket address: {}", e))?
            .port();

        let advertised_ip = host_ip_override.unwrap_or_else(detect_local_ip);
        let session_token = generate_session_token(rom_hash, actual_port);
        let clamped_delay = input_delay.clamp(1, 10);

        let invite_hash = InviteHash::new(
            SocketAddr::new(advertised_ip, actual_port),
            rom_hash,
            session_token,
            clamped_delay,
        );
        let localhost_hash = InviteHash::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), actual_port),
            rom_hash,
            session_token,
            clamped_delay,
        );

        let now = Instant::now();
        Ok(Self {
            socket,
            role: NetplayRole::Host,
            state: SessionState::Handshaking,
            invite_hash,
            localhost_hash,
            peer_addr: None,
            rom_hash,
            session_token,
            input_delay: clamped_delay,
            ignore_rom_hash,
            current_frame: 0,
            local_scheduled_up_to: None,
            queued_local_flags: 0,
            local_inputs: HashMap::new(),
            remote_inputs: HashMap::new(),
            local_checksums: HashMap::new(),
            remote_checksums: HashMap::new(),
            desync_count: 0,
            start_instant: now,
            last_packet_sent: now - Duration::from_secs(1),
            last_packet_received: now,
            last_ping_sent: now,
            rtt_ms: 0,
        })
    }

    /// Creates a new Peer session (Player 2) joining the host specified by `hash_str`.
    pub fn new_peer(
        hash_str: &str,
        local_rom_hash: u32,
        ignore_rom_hash: bool,
    ) -> Result<Self, String> {
        let invite_hash = InviteHash::decode(hash_str)?;

        if !ignore_rom_hash && invite_hash.rom_hash != local_rom_hash {
            return Err(format!(
                "ROM checksum mismatch! Invite hash expects ROM 0x{:08X}, but loaded ROM is 0x{:08X}. (Use --ignore-rom-hash to override)",
                invite_hash.rom_hash, local_rom_hash
            ));
        }

        let bind_addr: SocketAddr = if invite_hash.addr.is_ipv6() {
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
        } else {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
        };

        let socket = UdpSocket::bind(bind_addr)
            .map_err(|e| format!("Failed to bind local UDP socket: {}", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking UDP socket: {}", e))?;

        let now = Instant::now();
        let mut session = Self {
            socket,
            role: NetplayRole::Peer,
            state: SessionState::Handshaking,
            localhost_hash: invite_hash.clone(),
            peer_addr: Some(invite_hash.addr),
            rom_hash: local_rom_hash,
            session_token: invite_hash.session_token,
            input_delay: invite_hash.input_delay,
            ignore_rom_hash,
            invite_hash,
            current_frame: 0,
            local_scheduled_up_to: None,
            queued_local_flags: 0,
            local_inputs: HashMap::new(),
            remote_inputs: HashMap::new(),
            local_checksums: HashMap::new(),
            remote_checksums: HashMap::new(),
            desync_count: 0,
            start_instant: now,
            last_packet_sent: now - Duration::from_secs(1),
            last_packet_received: now,
            last_ping_sent: now,
            rtt_ms: 0,
        };

        session.send_handshake_req();
        Ok(session)
    }

    /// Local UDP port bound by this session.
    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    /// Queues a control flag (`INPUT_FLAG_RESET` or `INPUT_FLAG_PAUSE`) to be sent
    /// on the next scheduled lockstep frame.
    pub fn queue_local_flag(&mut self, flag: u8) {
        self.queued_local_flags |= flag;
    }

    /// Seeds initial delay frames `[0..input_delay)` with neutral inputs so lockstep begins immediately.
    fn initialize_delay_window(&mut self) {
        self.current_frame = 0;
        self.queued_local_flags = 0;
        self.local_inputs.clear();
        self.remote_inputs.clear();
        self.local_checksums.clear();
        self.remote_checksums.clear();
        for f in 0..(self.input_delay as u32) {
            self.local_inputs.insert(f, (0, 0));
            self.remote_inputs.insert(f, (0, 0));
        }
        self.local_scheduled_up_to = Some((self.input_delay as u32).saturating_sub(1));
    }

    fn elapsed_ms(&self) -> u32 {
        self.start_instant.elapsed().as_millis() as u32
    }

    /// Polls the UDP socket and advances the handshake or lockstep frame state.
    ///
    /// When `Connected`, schedules `(local_buttons, local_flags)` for `current_frame + input_delay`
    /// and returns `LockstepPoll::FrameReady` as soon as both players' inputs for `current_frame`
    /// have arrived.
    pub fn poll_step(
        &mut self,
        local_buttons: u8,
        local_flags: u8,
        state_checksum: u32,
    ) -> LockstepPoll {
        self.receive_packets();

        match &self.state {
            SessionState::Disconnected(reason) => {
                return LockstepPoll::Disconnected(reason.clone());
            }
            SessionState::Handshaking => {
                if self.role == NetplayRole::Peer
                    && self.last_packet_sent.elapsed() >= Duration::from_millis(80)
                {
                    self.send_handshake_req();
                }
                return LockstepPoll::WaitingForHandshake;
            }
            SessionState::Connected => {}
        }

        let target_frame = self.current_frame + (self.input_delay as u32);
        let already_scheduled = self
            .local_scheduled_up_to
            .map(|last| last >= target_frame)
            .unwrap_or(false);

        if !already_scheduled {
            let effective_flags = local_flags | self.queued_local_flags;
            self.queued_local_flags = 0;
            self.local_inputs
                .insert(target_frame, (local_buttons, effective_flags));
            self.local_checksums
                .insert(self.current_frame, state_checksum);
            self.verify_checksum(self.current_frame);
            self.local_scheduled_up_to = Some(target_frame);
            self.send_frame_input(target_frame);
        }

        // Send periodic ping for accurate RTT tracking
        if self.last_ping_sent.elapsed() >= Duration::from_millis(500) {
            self.send_ping();
        }

        // Check if both local and remote inputs are available for current_frame
        if let (Some(&(local_btn, local_flg)), Some(&(remote_btn, remote_flg))) = (
            self.local_inputs.get(&self.current_frame),
            self.remote_inputs.get(&self.current_frame),
        ) {
            let (p1_buttons, p2_buttons) = match self.role {
                NetplayRole::Host => (local_btn, remote_btn),
                NetplayRole::Peer => (remote_btn, local_btn),
            };
            let combined_flags = local_flg | remote_flg;
            let frame = self.current_frame;
            self.current_frame = self.current_frame.wrapping_add(1);

            // Prune old history older than 64 frames to keep memory bounded
            if frame > 64 {
                let cutoff = frame - 64;
                self.local_inputs.retain(|&k, _| k >= cutoff);
                self.remote_inputs.retain(|&k, _| k >= cutoff);
                self.local_checksums.retain(|&k, _| k >= cutoff);
                self.remote_checksums.retain(|&k, _| k >= cutoff);
            }

            LockstepPoll::FrameReady(SynchronizedFrame {
                frame,
                p1_buttons,
                p2_buttons,
                reset_triggered: (combined_flags & INPUT_FLAG_RESET) != 0,
                pause_toggled: (combined_flags & INPUT_FLAG_PAUSE) != 0,
            })
        } else {
            // Retransmit our latest input packet if we've been waiting > 12ms
            if self.last_packet_sent.elapsed() >= Duration::from_millis(12) {
                if let Some(latest) = self.local_scheduled_up_to {
                    self.send_frame_input(latest);
                }
            }
            LockstepPoll::WaitingForRemoteInput {
                frame: self.current_frame,
            }
        }
    }

    fn verify_checksum(&mut self, frame: u32) {
        if let (Some(&local_crc), Some(&remote_crc)) = (
            self.local_checksums.get(&frame),
            self.remote_checksums.get(&frame),
        ) {
            if local_crc != remote_crc {
                self.desync_count += 1;
                if self.desync_count <= 5 {
                    eprintln!(
                        "⚠️ [Netplay] State checksum mismatch at frame {}: local=0x{:08X}, remote=0x{:08X}",
                        frame, local_crc, remote_crc
                    );
                }
            }
        }
    }

    fn receive_packets(&mut self) {
        let mut buf = [0u8; 512];
        while let Ok((len, src_addr)) = self.socket.recv_from(&mut buf) {
            if len < 5 || &buf[0..4] != MAGIC {
                continue;
            }
            self.last_packet_received = Instant::now();
            let pkt_type = buf[4];
            let payload = &buf[5..len];

            match pkt_type {
                PKT_HANDSHAKE_REQ => self.handle_handshake_req(src_addr, payload),
                PKT_HANDSHAKE_ACK => self.handle_handshake_ack(src_addr, payload),
                PKT_HANDSHAKE_REJECT => self.handle_handshake_reject(payload),
                PKT_FRAME_INPUT => self.handle_frame_input(src_addr, payload),
                PKT_PING => self.handle_ping(src_addr, payload),
                PKT_PONG => self.handle_pong(payload),
                PKT_DISCONNECT if self.peer_addr == Some(src_addr) => {
                    self.state =
                        SessionState::Disconnected("Remote player disconnected".to_string());
                }
                _ => {}
            }
        }
    }

    fn handle_handshake_req(&mut self, src_addr: SocketAddr, payload: &[u8]) {
        if self.role != NetplayRole::Host || payload.len() < 6 {
            return;
        }
        let token = u16::from_be_bytes([payload[0], payload[1]]);
        let peer_rom_hash = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);

        if token != self.session_token {
            self.send_handshake_reject(src_addr, REJECT_INVALID_TOKEN);
            return;
        }

        if !self.ignore_rom_hash && peer_rom_hash != self.rom_hash {
            self.send_handshake_reject(src_addr, REJECT_ROM_MISMATCH);
            return;
        }

        if let Some(existing_peer) = self.peer_addr {
            if existing_peer != src_addr {
                self.send_handshake_reject(src_addr, REJECT_SESSION_FULL);
                return;
            }
        }

        if self.state == SessionState::Handshaking {
            self.peer_addr = Some(src_addr);
            self.initialize_delay_window();
            self.state = SessionState::Connected;
            println!(
                "✅ Player 2 connected from {}! Starting synchronized lockstep (delay: {} frames).",
                src_addr, self.input_delay
            );
        }

        self.send_handshake_ack(src_addr);
    }

    fn handle_handshake_ack(&mut self, src_addr: SocketAddr, payload: &[u8]) {
        if self.role != NetplayRole::Peer || payload.len() < 7 {
            return;
        }
        let token = u16::from_be_bytes([payload[0], payload[1]]);
        let negotiated_delay = payload[2].clamp(1, 10);
        let host_rom_hash = u32::from_be_bytes([payload[3], payload[4], payload[5], payload[6]]);

        if token != self.session_token {
            return;
        }
        if !self.ignore_rom_hash && host_rom_hash != self.rom_hash {
            self.state = SessionState::Disconnected(format!(
                "ROM mismatch with host (host: 0x{:08X}, local: 0x{:08X})",
                host_rom_hash, self.rom_hash
            ));
            return;
        }

        if self.state == SessionState::Handshaking {
            self.peer_addr = Some(src_addr);
            self.input_delay = negotiated_delay;
            self.initialize_delay_window();
            self.state = SessionState::Connected;
            println!(
                "✅ Connected to Host ({})! Starting synchronized lockstep (Player 2, delay: {} frames).",
                src_addr, self.input_delay
            );
        }
    }

    fn handle_handshake_reject(&mut self, payload: &[u8]) {
        if payload.len() < 7 {
            return;
        }
        let token = u16::from_be_bytes([payload[0], payload[1]]);
        if token != self.session_token {
            return;
        }
        let reason_code = payload[2];
        let host_rom = u32::from_be_bytes([payload[3], payload[4], payload[5], payload[6]]);
        let msg = match reason_code {
            REJECT_ROM_MISMATCH => format!(
                "Host rejected connection: ROM mismatch (Host ROM: 0x{:08X}, Local ROM: 0x{:08X})",
                host_rom, self.rom_hash
            ),
            REJECT_INVALID_TOKEN => "Host rejected connection: Invalid session token".to_string(),
            REJECT_SESSION_FULL => "Host rejected connection: Session is already full".to_string(),
            _ => "Host rejected connection".to_string(),
        };
        self.state = SessionState::Disconnected(msg);
    }

    fn handle_frame_input(&mut self, src_addr: SocketAddr, payload: &[u8]) {
        if payload.len() < 15 {
            return;
        }
        let token = u16::from_be_bytes([payload[0], payload[1]]);
        if token != self.session_token {
            return;
        }

        // If Peer missed HandshakeAck packet because of UDP drop, receiving a valid FrameInput
        // from Host also completes the handshake!
        if self.role == NetplayRole::Peer && self.state == SessionState::Handshaking {
            self.peer_addr = Some(src_addr);
            self.initialize_delay_window();
            self.state = SessionState::Connected;
        }

        if self.peer_addr != Some(src_addr) {
            return;
        }

        let latest_frame = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);
        let checksum_frame = u32::from_be_bytes([payload[6], payload[7], payload[8], payload[9]]);
        let remote_checksum =
            u32::from_be_bytes([payload[10], payload[11], payload[12], payload[13]]);
        let count = payload[14] as usize;

        if payload.len() < 15 + count * 2 {
            return;
        }

        self.remote_checksums
            .insert(checksum_frame, remote_checksum);
        self.verify_checksum(checksum_frame);

        for i in 0..count {
            if latest_frame >= (i as u32) {
                let f = latest_frame - (i as u32);
                let btn = payload[15 + i * 2];
                let flg = payload[15 + i * 2 + 1];
                self.remote_inputs.entry(f).or_insert((btn, flg));
            }
        }
    }

    fn handle_ping(&mut self, src_addr: SocketAddr, payload: &[u8]) {
        if payload.len() < 6 {
            return;
        }
        let token = u16::from_be_bytes([payload[0], payload[1]]);
        if token != self.session_token {
            return;
        }
        let mut pkt = [0u8; 11];
        pkt[0..4].copy_from_slice(MAGIC);
        pkt[4] = PKT_PONG;
        pkt[5..11].copy_from_slice(&payload[0..6]);
        let _ = self.socket.send_to(&pkt, src_addr);
    }

    fn handle_pong(&mut self, payload: &[u8]) {
        if payload.len() < 6 {
            return;
        }
        let token = u16::from_be_bytes([payload[0], payload[1]]);
        if token != self.session_token {
            return;
        }
        let sent_ms = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);
        let now_ms = self.elapsed_ms();
        if now_ms >= sent_ms {
            let sample = now_ms - sent_ms;
            self.rtt_ms = if self.rtt_ms == 0 {
                sample
            } else {
                (self.rtt_ms * 3 + sample) / 4
            };
        }
    }

    fn send_handshake_req(&mut self) {
        if let Some(peer) = self.peer_addr {
            let mut pkt = [0u8; 11];
            pkt[0..4].copy_from_slice(MAGIC);
            pkt[4] = PKT_HANDSHAKE_REQ;
            pkt[5..7].copy_from_slice(&self.session_token.to_be_bytes());
            pkt[7..11].copy_from_slice(&self.rom_hash.to_be_bytes());
            let _ = self.socket.send_to(&pkt, peer);
            self.last_packet_sent = Instant::now();
        }
    }

    fn send_handshake_ack(&mut self, peer: SocketAddr) {
        let mut pkt = [0u8; 12];
        pkt[0..4].copy_from_slice(MAGIC);
        pkt[4] = PKT_HANDSHAKE_ACK;
        pkt[5..7].copy_from_slice(&self.session_token.to_be_bytes());
        pkt[7] = self.input_delay;
        pkt[8..12].copy_from_slice(&self.rom_hash.to_be_bytes());
        let _ = self.socket.send_to(&pkt, peer);
        self.last_packet_sent = Instant::now();
    }

    fn send_handshake_reject(&mut self, peer: SocketAddr, reason: u8) {
        let mut pkt = [0u8; 12];
        pkt[0..4].copy_from_slice(MAGIC);
        pkt[4] = PKT_HANDSHAKE_REJECT;
        pkt[5..7].copy_from_slice(&self.session_token.to_be_bytes());
        pkt[7] = reason;
        pkt[8..12].copy_from_slice(&self.rom_hash.to_be_bytes());
        let _ = self.socket.send_to(&pkt, peer);
    }

    fn send_frame_input(&mut self, latest_frame: u32) {
        let peer = match self.peer_addr {
            Some(p) => p,
            None => return,
        };

        let mut pkt = Vec::with_capacity(20 + MAX_INPUT_HISTORY * 2);
        pkt.extend_from_slice(MAGIC);
        pkt.push(PKT_FRAME_INPUT);
        pkt.extend_from_slice(&self.session_token.to_be_bytes());
        pkt.extend_from_slice(&latest_frame.to_be_bytes());

        let checksum_frame = self.current_frame;
        let state_checksum = self
            .local_checksums
            .get(&checksum_frame)
            .copied()
            .unwrap_or(0);
        pkt.extend_from_slice(&checksum_frame.to_be_bytes());
        pkt.extend_from_slice(&state_checksum.to_be_bytes());

        let max_count = (latest_frame as usize + 1).min(MAX_INPUT_HISTORY);
        pkt.push(max_count as u8);

        for i in 0..max_count {
            let f = latest_frame - (i as u32);
            let (btn, flg) = self.local_inputs.get(&f).copied().unwrap_or((0, 0));
            pkt.push(btn);
            pkt.push(flg);
        }

        let _ = self.socket.send_to(&pkt, peer);
        self.last_packet_sent = Instant::now();
    }

    fn send_ping(&mut self) {
        if let Some(peer) = self.peer_addr {
            let mut pkt = [0u8; 11];
            pkt[0..4].copy_from_slice(MAGIC);
            pkt[4] = PKT_PING;
            pkt[5..7].copy_from_slice(&self.session_token.to_be_bytes());
            pkt[7..11].copy_from_slice(&self.elapsed_ms().to_be_bytes());
            let _ = self.socket.send_to(&pkt, peer);
            self.last_ping_sent = Instant::now();
        }
    }

    /// Sends a clean disconnect notification to the remote peer.
    pub fn disconnect(&mut self) {
        if let Some(peer) = self.peer_addr {
            let mut pkt = [0u8; 7];
            pkt[0..4].copy_from_slice(MAGIC);
            pkt[4] = PKT_DISCONNECT;
            pkt[5..7].copy_from_slice(&self.session_token.to_be_bytes());
            let _ = self.socket.send_to(&pkt, peer);
        }
        self.state = SessionState::Disconnected("Local disconnect".to_string());
    }

    /// Formats a concise status string for the SDL2 window title bar.
    pub fn status_badge(&self) -> String {
        match &self.state {
            SessionState::Handshaking => match self.role {
                NetplayRole::Host => {
                    format!("🌐 Waiting for P2 | Hash: {}", self.invite_hash.encode())
                }
                NetplayRole::Peer => {
                    format!("🌐 Connecting to {}...", self.invite_hash.addr)
                }
            },
            SessionState::Connected => {
                let role_str = match self.role {
                    NetplayRole::Host => "P1 Host",
                    NetplayRole::Peer => "P2 Peer",
                };
                format!(
                    "🌐 {} | {}ms | Delay {}f",
                    role_str, self.rtt_ms, self.input_delay
                )
            }
            SessionState::Disconnected(reason) => {
                format!("🌐 Disconnected: {}", reason)
            }
        }
    }
}

impl Drop for NetplaySession {
    fn drop(&mut self) {
        if self.state == SessionState::Connected {
            self.disconnect();
        }
    }
}

fn generate_session_token(rom_hash: u32, port: u16) -> u16 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0x1357_9BDF);
    let mixed = nanos ^ rom_hash ^ ((port as u32) << 16);
    let token = ((mixed >> 16) as u16) ^ (mixed as u16);
    if token == 0 {
        0x4E53
    } else {
        token
    }
}
