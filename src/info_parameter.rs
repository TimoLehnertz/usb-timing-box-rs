//! [`INFOGET`](https://www.raceresult.com/en/support/kb?id=420-Command-INFOGET) parameter IDs.
//!
//! Reply format: `[parameter-id:2];[value:{2,4}]` (see also
//! [Quick Start — INFOGET](https://www.raceresult.com/en/support/kbexport2?id=21)).

use crate::{
    commands::{BatteryState, BoxType, LoopStatus},
    error::Error,
    utils::{parse_hex_u8, parse_hex_u16, parse_signed_hex_i8},
};

/// General information parameters queryable via `INFOGET;[id]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InfoParameter {
    /// Decoder ID (4 hex digits, e.g. `1387` → transponder prefix `A-4999`).
    DecoderId = 0x01,
    /// Firmware version (`18` → v2.4).
    FirmwareVersion = 0x02,
    /// Hardware version (`18` → v2.4).
    HardwareVersion = 0x03,
    /// Device type (`28` = USB timing box).
    BoxType = 0x04,
    /// Battery voltage in volts (`28` → 4.0 V).
    BatteryVoltage = 0x05,
    /// Battery state (fault / charging / …).
    BatteryState = 0x07,
    /// Battery level 0–100 %.
    BatteryLevel = 0x08,
    /// Internal temperature (FW 2.5: 0–100 °C unsigned; FW 2.6: signed hex).
    InternalTemperature = 0x09,
    /// Supply voltage in volts.
    SupplyVoltage = 0x0a,
    /// Loop status.
    LoopStatus = 0x0b,
    /// Build revision string (internal use).
    BuiltRevision = 0x0c,
    /// Measured loop power 0–100 % (may differ slightly from configured power).
    MeasuredLoopPower = 0x0d,
    /// This is documented but in my testing the usb timing box doesn't seem to support it.
    /// Channel noise 0–10 (FW 2.6+, same scale as site survey).
    NoiseStatus = 0x0e,
}

impl InfoParameter {
    pub const fn id(self) -> u8 {
        self as u8
    }
}

pub(crate) fn parse_decoder_id(value: &str) -> Result<u16, Error> {
    parse_hex_u16(value)
}

pub(crate) fn parse_version_tenths(value: &str) -> Result<f32, Error> {
    Ok(f32::from(parse_hex_u8(value)?) / 10.0)
}

pub(crate) fn parse_voltage_tenths(value: &str) -> Result<f32, Error> {
    Ok(f32::from(parse_hex_u8(value)?) / 10.0)
}

pub(crate) fn parse_box_type(value: &str) -> Result<BoxType, Error> {
    Ok(BoxType::from_raw(parse_hex_u8(value)?))
}

pub(crate) fn parse_battery_state(value: &str) -> Result<BatteryState, Error> {
    Ok(BatteryState::from_raw(parse_hex_u8(value)?))
}

pub(crate) fn parse_battery_level_percent(value: &str) -> Result<u8, Error> {
    let level = parse_hex_u8(value)?;
    if level > 100 {
        return Err(Error::Protocol(format!("battery level out of range: {level}")));
    }
    Ok(level)
}

pub(crate) fn parse_internal_temperature_fw25(value: &str) -> Result<u8, Error> {
    let temp = parse_hex_u8(value)?;
    if temp > 100 {
        return Err(Error::Protocol(format!("internal temperature out of range: {temp}")));
    }
    Ok(temp)
}

pub(crate) fn parse_internal_temperature_fw26(value: &str) -> Result<i8, Error> {
    parse_signed_hex_i8(value)
}

pub(crate) fn parse_loop_status(value: &str) -> Result<LoopStatus, Error> {
    Ok(LoopStatus::from_raw(parse_hex_u8(value)?))
}

pub(crate) fn parse_measured_loop_power_percent(value: &str) -> Result<u8, Error> {
    let power = parse_hex_u8(value)?;
    if power > 0x64 {
        return Err(Error::Protocol(format!("measured loop power out of range: {power}")));
    }
    Ok(power)
}

pub(crate) fn parse_noise_status(value: &str) -> Result<u8, Error> {
    let noise = parse_hex_u8(value)?;
    if noise > 0x0a {
        return Err(Error::Protocol(format!("noise status out of range: {noise}")));
    }
    Ok(noise)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::BoxType;

    #[test]
    fn parse_info_decoder_id_doc_example() {
        assert_eq!(parse_decoder_id("1387").unwrap(), 0x1387);
    }

    #[test]
    fn parse_info_firmware_version_doc_example() {
        assert!((parse_version_tenths("18").unwrap() - 2.4).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_info_battery_voltage_doc_example() {
        assert!((parse_voltage_tenths("28").unwrap() - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_info_box_type_usb_timing_box() {
        assert_eq!(parse_box_type("28").unwrap(), BoxType::UsbTimingBox);
    }

    #[test]
    fn parse_info_battery_level_doc_example() {
        assert_eq!(parse_battery_level_percent("4e").unwrap(), 78);
    }
}
