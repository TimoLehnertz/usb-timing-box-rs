use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Error;

pub fn parse_hex_u8(s: &str) -> Result<u8, Error> {
    u8::from_str_radix(s.trim(), 16).map_err(|e| Error::Protocol(format!("invalid hex u8 '{s}': {e}")))
}

pub fn parse_hex_u16(s: &str) -> Result<u16, Error> {
    u16::from_str_radix(s.trim(), 16).map_err(|e| Error::Protocol(format!("invalid hex u16 '{s}': {e}")))
}

pub fn parse_hex_u32(s: &str) -> Result<u32, Error> {
    u32::from_str_radix(s.trim(), 16).map_err(|e| Error::Protocol(format!("invalid hex u32 '{s}': {e}")))
}

pub fn parse_hex_u64(s: &str) -> Result<u64, Error> {
    u64::from_str_radix(s.trim(), 16).map_err(|e| Error::Protocol(format!("invalid hex u64 '{s}': {e}")))
}

pub fn parse_signed_hex_i8(s: &str) -> Result<i8, Error> {
    let raw = s.trim();
    let (sign, digits) = if let Some(rest) = raw.strip_prefix('+') {
        (1_i16, rest)
    } else if let Some(rest) = raw.strip_prefix('-') {
        (-1_i16, rest)
    } else {
        (1_i16, raw)
    };
    let magnitude =
        i16::from_str_radix(digits, 16).map_err(|e| Error::Protocol(format!("invalid signed hex i8 '{s}': {e}")))?;
    let value = sign * magnitude;
    i8::try_from(value).map_err(|e| Error::Protocol(format!("signed hex i8 out of range '{s}': {e}")))
}

pub fn hex2(value: u8) -> String {
    format!("{value:02x}")
}

pub fn hex8(value: u32) -> String {
    format!("{value:08x}")
}

pub fn unix_time_now() -> Result<u32, Error> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Protocol(format!("system time before UNIX_EPOCH: {e}")))?
        .as_secs();

    u32::try_from(secs).map_err(|e| Error::Protocol(format!("unix time does not fit u32: {e}")))
}
