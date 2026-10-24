use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs;
use std::path::Path;

/// Parses a u16 address from either a JSON integer (e.g. `170`) or a string
/// (e.g. `"0x00AA"`, `"$00AA"`, `"00AA"`, or `"170"`).
pub fn parse_address_str(s: &str) -> Result<u16, String> {
    let trimmed = s.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .or_else(|| trimmed.strip_prefix('$'))
    {
        u16::from_str_radix(hex.trim(), 16)
            .map_err(|e| format!("Invalid hex address '{}': {}", s, e))
    } else if trimmed
        .chars()
        .any(|c| c.is_ascii_hexdigit() && !c.is_ascii_digit())
        || (trimmed.len() == 4 && trimmed.starts_with('0'))
    {
        u16::from_str_radix(trimmed, 16).map_err(|e| format!("Invalid hex address '{}': {}", s, e))
    } else {
        trimmed
            .parse::<u16>()
            .or_else(|_| u16::from_str_radix(trimmed, 16))
            .map_err(|e| format!("Invalid address '{}': {}", s, e))
    }
}

/// Parses a u8 value from either a JSON integer (e.g. `19`) or a string
/// (e.g. `"0x13"`, `"$13"`, or `"19"`).
pub fn parse_value_str(s: &str) -> Result<u8, String> {
    let trimmed = s.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .or_else(|| trimmed.strip_prefix('$'))
    {
        u8::from_str_radix(hex.trim(), 16).map_err(|e| format!("Invalid hex value '{}': {}", s, e))
    } else if trimmed
        .chars()
        .any(|c| c.is_ascii_hexdigit() && !c.is_ascii_digit())
    {
        u8::from_str_radix(trimmed, 16).map_err(|e| format!("Invalid hex value '{}': {}", s, e))
    } else {
        trimmed
            .parse::<u8>()
            .map_err(|e| format!("Invalid byte value '{}': {}", s, e))
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawAddress {
    Num(u16),
    Str(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawByte {
    Num(u8),
    Str(String),
}

fn serialize_address<S: Serializer>(addr: &u16, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("0x{:04X}", addr))
}

fn deserialize_address<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    match RawAddress::deserialize(deserializer)? {
        RawAddress::Num(n) => Ok(n),
        RawAddress::Str(s) => parse_address_str(&s).map_err(serde::de::Error::custom),
    }
}

fn serialize_byte<S: Serializer>(val: &u8, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("0x{:02X}", val))
}

fn deserialize_byte<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    match RawByte::deserialize(deserializer)? {
        RawByte::Num(n) => Ok(n),
        RawByte::Str(s) => parse_value_str(&s).map_err(serde::de::Error::custom),
    }
}

fn serialize_opt_byte<S: Serializer>(val: &Option<u8>, serializer: S) -> Result<S::Ok, S::Error> {
    match val {
        Some(b) => serializer.serialize_some(&format!("0x{:02X}", b)),
        None => serializer.serialize_none(),
    }
}

fn deserialize_opt_byte<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u8>, D::Error> {
    let opt = Option::<RawByte>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(RawByte::Num(n)) => Ok(Some(n)),
        Some(RawByte::Str(s)) => parse_value_str(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

fn default_enabled() -> bool {
    true
}

/// Represents a single memory cheat (writing/freezing a specific memory address to a value).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheatEntry {
    /// Human-readable description of the cheat.
    #[serde(default)]
    pub name: String,
    /// Target 16-bit CPU memory address (e.g. `0x00AA` for Contra Player 1 weapon).
    #[serde(
        serialize_with = "serialize_address",
        deserialize_with = "deserialize_address"
    )]
    pub address: u16,
    /// 8-bit value to write/freeze at `address` (e.g. `0x13` for Spread Gun + Rapid Fire).
    #[serde(
        serialize_with = "serialize_byte",
        deserialize_with = "deserialize_byte"
    )]
    pub value: u8,
    /// Optional conditional comparison byte: if set, the cheat only applies when the
    /// current value at `address` matches `compare`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt_byte",
        deserialize_with = "deserialize_opt_byte"
    )]
    pub compare: Option<u8>,
    /// Whether this cheat is active (defaults to `true`).
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl CheatEntry {
    pub fn new(name: impl Into<String>, address: u16, value: u8) -> Self {
        Self {
            name: name.into(),
            address,
            value,
            compare: None,
            enabled: true,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawCheatsConfig {
    Object {
        #[serde(default)]
        cheats: Vec<CheatEntry>,
    },
    List(Vec<CheatEntry>),
}

/// Configuration file holding a list of memory cheats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheatsConfig {
    pub cheats: Vec<CheatEntry>,
}

impl<'de> Deserialize<'de> for CheatsConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match RawCheatsConfig::deserialize(deserializer)? {
            RawCheatsConfig::Object { cheats } => Ok(CheatsConfig { cheats }),
            RawCheatsConfig::List(cheats) => Ok(CheatsConfig { cheats }),
        }
    }
}

impl Default for CheatsConfig {
    /// Default configuration includes Contra Spread Gun + Rapid Fire for Player 1 & Player 2.
    fn default() -> Self {
        Self::contra_spread_rapid()
    }
}

impl CheatsConfig {
    /// Preset cheats for Contra (NES):
    /// - `0x00AA` = `0x13`: Player 1 Spread Gun (`0x03`) + Rapid Fire (`0x10`)
    /// - `0x00AB` = `0x13`: Player 2 Spread Gun (`0x03`) + Rapid Fire (`0x10`)
    pub fn contra_spread_rapid() -> Self {
        Self {
            cheats: vec![
                CheatEntry {
                    name: "Contra - Player 1 Spread Gun + Rapid Fire".to_string(),
                    address: 0x00AA,
                    value: 0x13,
                    compare: None,
                    enabled: true,
                },
                CheatEntry {
                    name: "Contra - Player 2 Spread Gun + Rapid Fire".to_string(),
                    address: 0x00AB,
                    value: 0x13,
                    compare: None,
                    enabled: true,
                },
            ],
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let p = path.as_ref();
        let content = fs::read_to_string(p).map_err(|e| {
            format!(
                "Failed to read cheats configuration file '{}': {}",
                p.display(),
                e
            )
        })?;
        Self::from_json_str(&content).map_err(|e| {
            format!(
                "Failed to parse cheats configuration from '{}': {}",
                p.display(),
                e
            )
        })
    }

    pub fn from_json_str(json_str: &str) -> Result<Self, String> {
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let p = path.as_ref();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize cheats configuration: {}", e))?;
        fs::write(p, json).map_err(|e| {
            format!(
                "Failed to write cheats configuration to '{}': {}",
                p.display(),
                e
            )
        })
    }

    /// Loads cheats from an explicit file path, or `./cheats.json` if present.
    pub fn load_or_empty<P: AsRef<Path>>(path: Option<P>) -> (Self, Option<String>) {
        if let Some(p) = path {
            let path_ref = p.as_ref();
            match Self::from_file(path_ref) {
                Ok(cfg) => (cfg, Some(path_ref.display().to_string())),
                Err(e) => {
                    eprintln!("Warning: {}, continuing without cheats", e);
                    (Self { cheats: Vec::new() }, None)
                }
            }
        } else {
            let default_path = Path::new("cheats.json");
            if default_path.exists() {
                match Self::from_file(default_path) {
                    Ok(cfg) => (cfg, Some("cheats.json".to_string())),
                    Err(e) => {
                        eprintln!("Warning: {}, continuing without cheats", e);
                        (Self { cheats: Vec::new() }, None)
                    }
                }
            } else {
                (Self { cheats: Vec::new() }, None)
            }
        }
    }

    pub fn active_cheats(&self) -> Vec<CheatEntry> {
        self.cheats.iter().filter(|c| c.enabled).cloned().collect()
    }
}
