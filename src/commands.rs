use crate::{
    Error,
    firmware::{FW25_TICKS_PER_SECOND, FW26_TICKS_PER_SECOND, Fw25, Fw26},
    passing::{PassingBatch, PassingFw25, PassingFw26},
    utils::{parse_hex_u8, parse_hex_u16, parse_hex_u32, parse_hex_u64, parse_signed_hex_i8},
};

#[derive(Debug, Clone)]
pub struct CommandResponse {
    pub command: String,

    /// 0 => success, no error
    /// 10-1f => command specific error, see command description
    /// ff => error unknown command or parameter(s), unless caught by return codes 10-1f
    pub return_code: u8,

    pub data_lines: Vec<String>,
}

impl CommandResponse {
    pub fn single_data_line(&self) -> Option<&str> {
        if self.data_lines.len() == 1 { self.data_lines.first().map(String::as_str) } else { None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PassingInfo {
    /// Number of passings in buffer.
    pub count: u16,

    /// Index of first passing in buffer.
    pub start_id: u32,

    /// Timestamp of first passing in buffer (low 32 bits in FW 2.6 mode).
    pub start_timestamp: u32,

    /// Index of last passing in buffer.
    pub last_id: u32,

    /// Timestamp of last passing in buffer (low 32 bits in FW 2.6 mode).
    pub last_timestamp: u32,
}

impl PassingInfo {
    /// Parses a `PASSINGINFOGET` data line: `[Count:4];[StartID:8];[StartTimeStamp:8];[LastID:8];[LastTimeStamp:8]`.
    pub fn from_line(line: &str) -> Result<Self, Error> {
        let mut parts = line.split(';');
        Ok(Self {
            count: parts
                .next()
                .ok_or_else(|| Error::Protocol("PASSINGINFOGET missing count".to_string()))
                .and_then(parse_hex_u16)?,
            start_id: parts
                .next()
                .ok_or_else(|| Error::Protocol("PASSINGINFOGET missing start_id".to_string()))
                .and_then(parse_hex_u32)?,
            start_timestamp: parts
                .next()
                .ok_or_else(|| Error::Protocol("PASSINGINFOGET missing start_timestamp".to_string()))
                .and_then(parse_hex_u32)?,
            last_id: parts
                .next()
                .ok_or_else(|| Error::Protocol("PASSINGINFOGET missing last_id".to_string()))
                .and_then(parse_hex_u32)?,
            last_timestamp: parts
                .next()
                .ok_or_else(|| Error::Protocol("PASSINGINFOGET missing last_timestamp".to_string()))
                .and_then(parse_hex_u32)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum PassingGetResult<F: crate::passing::FirmwarePassing> {
    Ok(PassingBatch<F>),

    /// The official docs say: "In case of this error, you need to execute `PASSINGGET;[MinStartIndex]` again."

    /// However this also gets returned when the start index is too large.

    /// Gets returned when the start index is not found in the internal buffer.
    StartIndexNotFound {
        /// indicating the lowest StartIndex that is available in memory
        min_start_index: u32,
    },

    /// The Box is running in a Mode which does not allow pulling passings from it.

    /// Most likely this is a USB Timing Box in Repeat Mode. The User needs to switch mode.
    WrongMode,
}

/// Parsed beacon record (FW 2.5 standard format, 17 fields).

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct BeaconRecordFw25 {
    pub active_device_id: u16,

    pub loop_status: LoopStatus,

    pub mode: BeaconMode,

    pub loop_data: u8,

    pub loop_power: u8,

    pub channel_id: u8,

    pub loop_id: u8,

    pub power_connection: PowerConnection,

    pub power_status_raw: u8,

    pub beacon_index: u16,

    /// Internal box time ticks (32-bit, 256 Hz).
    pub time_ticks: u32,

    pub channel_noise_avg: u8,

    pub transponder_energy_detect: u8,

    pub beacon_energy_detect: u8,
}

impl BeaconRecordFw25 {
    pub fn from_line(line: &str) -> Result<Self, Error> {
        let fields: Vec<&str> = line.split(';').collect();

        if fields.len() != 17 {
            return Err(Error::Protocol(format!(
                "FW2.5 beacon expected 17 fields, got {} (line: {line})",
                fields.len()
            )));
        }

        Ok(Self {
            active_device_id: parse_hex_u16(fields[0])?,

            loop_status: LoopStatus::from_raw(parse_hex_u8(fields[1])?),

            mode: BeaconMode::from_raw(parse_hex_u8(fields[2])?),

            loop_data: parse_hex_u8(fields[3])?,

            loop_power: parse_hex_u8(fields[4])?,

            channel_id: parse_hex_u8(fields[5])?,

            loop_id: parse_hex_u8(fields[6])?,

            power_connection: PowerConnection::from_raw(parse_hex_u8(fields[7])?),

            power_status_raw: parse_hex_u8(fields[8])?,

            beacon_index: parse_hex_u16(fields[9])?,

            time_ticks: parse_hex_u32(fields[10])?,

            channel_noise_avg: parse_hex_u8(fields[12])?,

            transponder_energy_detect: parse_hex_u8(fields[14])?,

            beacon_energy_detect: parse_hex_u8(fields[16])?,
        })
    }

    pub fn display_channel(&self) -> u8 {
        self.channel_id.saturating_add(1)
    }

    pub fn display_loop_id(&self) -> u8 {
        self.loop_id.saturating_add(1)
    }

    pub fn transponder_rssi_dbm(&self) -> i16 {
        -90 + i16::from(self.transponder_energy_detect)
    }

    pub fn beacon_rssi_dbm(&self) -> i16 {
        -90 + i16::from(self.beacon_energy_detect)
    }

    pub fn power_status(&self) -> BeaconPowerStatus {
        match self.power_connection {
            PowerConnection::Battery => BeaconPowerStatus::BatteryHours(self.power_status_raw),

            _ => BeaconPowerStatus::Voltage(self.power_status_raw as f32 / 10.0),
        }
    }
}

/// Parsed beacon record (FW 2.6 format, 26 fields).

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct BeaconRecordFw26 {
    pub active_device_id: u16,

    pub loop_status: LoopStatus,

    pub mode: BeaconMode,

    pub loop_data: u8,

    pub loop_power: u8,

    pub channel_id: u8,

    pub loop_id: u8,

    pub power_connection: PowerConnection,

    pub power_status_raw: u8,

    pub beacon_index: u16,

    /// Internal box time ticks (40-bit, 2048 Hz, 10 hex digits).
    pub time_ticks: u64,

    pub beacon_version: u8,

    pub channel_noise_avg: u8,

    pub transponder_energy_detect: u8,

    pub beacon_energy_detect: u8,

    pub beacon_success_rate: u8,

    pub fw_version_raw: u8,

    pub box_type: BoxType,

    pub box_mode_raw: u8,

    pub temperature_celsius: i8,

    pub buffer_overflow: bool,

    pub buffer_fill_state_percent: u8,

    pub avg_transponder_retries: u8,

    pub avg_repeat_retries: u8,
}

impl BeaconRecordFw26 {
    pub fn from_line(line: &str) -> Result<Self, Error> {
        let fields: Vec<&str> = line.split(';').collect();

        if fields.len() != 26 {
            return Err(Error::Protocol(format!(
                "FW2.6 beacon expected 26 fields, got {} (line: {line})",
                fields.len()
            )));
        }

        Ok(Self {
            active_device_id: parse_hex_u16(fields[0])?,

            loop_status: LoopStatus::from_raw(parse_hex_u8(fields[1])?),

            mode: BeaconMode::from_raw(parse_hex_u8(fields[2])?),

            loop_data: parse_hex_u8(fields[3])?,

            loop_power: parse_hex_u8(fields[4])?,

            channel_id: parse_hex_u8(fields[5])?,

            loop_id: parse_hex_u8(fields[6])?,

            power_connection: PowerConnection::from_raw(parse_hex_u8(fields[7])?),

            power_status_raw: parse_hex_u8(fields[8])?,

            beacon_index: parse_hex_u16(fields[9])?,

            time_ticks: parse_hex_u64(fields[10])?,

            beacon_version: parse_hex_u8(fields[11])?,

            channel_noise_avg: parse_hex_u8(fields[12])?,

            transponder_energy_detect: parse_hex_u8(fields[14])?,

            beacon_energy_detect: parse_hex_u8(fields[16])?,

            beacon_success_rate: parse_hex_u8(fields[17])?,

            fw_version_raw: parse_hex_u8(fields[18])?,

            box_type: BoxType::from_raw(parse_hex_u8(fields[19])?),

            box_mode_raw: parse_hex_u8(fields[20])?,

            temperature_celsius: parse_signed_hex_i8(fields[21])?,

            buffer_overflow: parse_hex_u8(fields[22])? != 0,

            buffer_fill_state_percent: parse_hex_u8(fields[23])?,

            avg_transponder_retries: parse_hex_u8(fields[24])?,

            avg_repeat_retries: parse_hex_u8(fields[25])?,
        })
    }

    pub fn display_channel(&self) -> u8 {
        self.channel_id.saturating_add(1)
    }

    pub fn display_loop_id(&self) -> u8 {
        self.loop_id.saturating_add(1)
    }

    pub fn transponder_rssi_dbm(&self) -> i16 {
        -90 + i16::from(self.transponder_energy_detect)
    }

    pub fn beacon_rssi_dbm(&self) -> i16 {
        -90 + i16::from(self.beacon_energy_detect)
    }

    pub fn power_status(&self) -> BeaconPowerStatus {
        match self.power_connection {
            PowerConnection::Battery => BeaconPowerStatus::BatteryHours(self.power_status_raw),

            _ => BeaconPowerStatus::Voltage(self.power_status_raw as f32 / 10.0),
        }
    }

    /// Converts raw FW version encoding (`26` => `2.6`) to a float.
    pub fn fw_version(&self) -> f32 {
        self.fw_version_raw as f32 / 10.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum BeaconPowerStatus {
    BatteryHours(u8),
    Voltage(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum LoopStatus {
    /// Called LoopOk instead of Ok to not conflict with [core::result::Result::Ok]
    LoopOk,
    Fault,
    Limit,
    OverVoltage,
    Unknown(u8),
}

impl LoopStatus {
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0x00 => Self::LoopOk,
            0x01 => Self::Fault,
            0x02 => Self::Limit,
            0x04 => Self::OverVoltage,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum BeaconMode {
    TimingMode,
    StoreMode,
    RepeatMode,
    RepeatImpulseMode,
    NoTimingDevice,
    TrackingStart,
    TrackingStop,
    Unknown(u8),
}

impl BeaconMode {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0x00 => Self::TimingMode,
            0x02 => Self::StoreMode,
            0x03 => Self::RepeatMode,
            0x04 => Self::RepeatImpulseMode,
            0x10 => Self::NoTimingDevice,
            0x21 => Self::TrackingStart,
            0x22 => Self::TrackingStop,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum PowerConnection {
    Power12V,
    Usb,
    Battery,
    Unknown(u8),
}

impl PowerConnection {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Power12V,
            1 => Self::Usb,
            2 => Self::Battery,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum BoxType {
    ActiveExtension,
    LoopBox,
    ManagementBox,
    UsbTimingBox,
    Ubidium,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    Fault,
    Charging,
    ReducedCharging,
    Discharging,
    Unknown(u8),
}

impl BatteryState {
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Fault,
            1 => Self::Charging,
            2 => Self::ReducedCharging,
            3 => Self::Discharging,
            other => Self::Unknown(other),
        }
    }
}

impl BoxType {
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0x0a => Self::ActiveExtension,

            0x14 => Self::LoopBox,

            0x1e => Self::ManagementBox,

            0x28 => Self::UsbTimingBox,

            0x50 => Self::Ubidium,

            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum OperationMode {
    /// for testing transponders, showing total number of passings and last transponder ID on the display.

    /// CH# and Loop# are automatically set, avoiding any collisions with other r|r Active Systems.
    Kiosk = 0x05,

    /// Standard Timing Mode. CH# and Loop# are selected by computer.
    Timing = 0x06,

    /// Transponder will create a stored passing. V2 transponders will send a passing copy to this USB Timing Box.
    StoreAndCopy = 0x07,

    /// USB Timing Box is in repeat mode. Getting passings is not possible in this mode.
    RepeatImpulse = 0x08,
}

/// Epoch reference pair (FW 2.5, 256 Hz timestamps).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct EpochReferenceFw25 {
    pub unix_time_seconds: u32,

    /// Reference box timestamp in 256th-second ticks (32-bit).
    pub timestamp_ticks: u32,
}

impl EpochReferenceFw25 {
    pub fn parse(line: &str) -> Result<Self, Error> {
        let mut parts = line.split(';');

        let unix_time_seconds =
            parts.next().ok_or_else(|| Error::Protocol("missing epoch seconds".to_string())).and_then(parse_hex_u32)?;

        let timestamp_ticks = parts
            .next()
            .ok_or_else(|| Error::Protocol("missing timestamp ticks".to_string()))
            .and_then(parse_hex_u32)?;

        Ok(Self { unix_time_seconds, timestamp_ticks })
    }

    pub fn passing_time_seconds(&self, passing_timestamp_ticks: u32) -> f64 {
        let delta = passing_timestamp_ticks.wrapping_sub(self.timestamp_ticks) as f64;

        self.unix_time_seconds as f64 + (delta / f64::from(FW25_TICKS_PER_SECOND))
    }
}

/// Epoch reference pair (FW 2.6, 2048 Hz / 40-bit timestamps).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct EpochReferenceFw26 {
    pub unix_time_seconds: u32,

    /// Reference box timestamp in 2048th-second ticks (40-bit, 10 hex digits on the wire).
    pub timestamp_ticks: u64,
}

impl EpochReferenceFw26 {
    pub fn parse(line: &str) -> Result<Self, Error> {
        let mut parts = line.split(';');

        let unix_time_seconds =
            parts.next().ok_or_else(|| Error::Protocol("missing epoch seconds".to_string())).and_then(parse_hex_u32)?;

        let timestamp_ticks = parts
            .next()
            .ok_or_else(|| Error::Protocol("missing timestamp ticks".to_string()))
            .and_then(parse_hex_u64)?;

        Ok(Self { unix_time_seconds, timestamp_ticks })
    }

    pub fn passing_time_seconds(self, passing_timestamp_ticks: u64) -> f64 {
        let delta = passing_timestamp_ticks.wrapping_sub(self.timestamp_ticks) as f64;

        self.unix_time_seconds as f64 + (delta / f64::from(FW26_TICKS_PER_SECOND))
    }
}

fn parse_passing_get_header(start_index: u32, response: &CommandResponse) -> Result<u8, Error> {
    let header =
        response.data_lines.first().ok_or_else(|| Error::Protocol("PASSINGGET missing header line".to_string()))?;

    let mut parts = header.split(';');

    let echoed_start = parts
        .next()
        .ok_or_else(|| Error::Protocol("PASSINGGET missing echoed start".to_string()))
        .and_then(parse_hex_u32)?;

    if echoed_start != start_index {
        return Err(Error::Protocol(format!(
            "PASSINGGET echoed start mismatch: header={echoed_start}, requested={start_index}"
        )));
    }

    parts.next().ok_or_else(|| Error::Protocol("PASSINGGET missing count".to_string())).and_then(parse_hex_u8)
}

fn passing_get_error(start_index: u32, response: CommandResponse) -> Result<PassingGetResult<Fw25>, Error> {
    match response.return_code {
        0x10 => {
            let line = response
                .single_data_line()
                .ok_or_else(|| Error::Protocol("PASSINGGET;10 missing data line".to_string()))?;

            let mut parts = line.split(';');

            let echoed_start = parts
                .next()
                .ok_or_else(|| Error::Protocol("PASSINGGET;10 missing echoed start".to_string()))
                .and_then(parse_hex_u32)?;

            if echoed_start != start_index {
                return Err(Error::Protocol(format!(
                    "PASSINGGET;10 echoed start mismatch: header={echoed_start}, requested={start_index}"
                )));
            }

            let min_start_index = parts
                .next()
                .ok_or_else(|| Error::Protocol("PASSINGGET;10 missing min start".to_string()))
                .and_then(parse_hex_u32)?;

            Ok(PassingGetResult::StartIndexNotFound { min_start_index })
        }
        0x11 => Ok(PassingGetResult::WrongMode),
        other => {
            Err(Error::CommandFailed { command: response.command, return_code: other, data_lines: response.data_lines })
        }
    }
}

pub(crate) fn passing_get_from_response_fw25(
    start_index: u32,
    response: CommandResponse,
) -> Result<PassingGetResult<Fw25>, Error> {
    match response.return_code {
        0x00 => {
            let count = parse_passing_get_header(start_index, &response)?;
            let mut passings = Vec::new();
            for line in response.data_lines.iter().skip(1) {
                passings.push(PassingFw25::from_line(line)?);
            }
            if passings.len() != count as usize {
                return Err(Error::Protocol(format!(
                    "PASSINGGET count mismatch: header={count}, records={}",
                    passings.len()
                )));
            }
            Ok(PassingGetResult::Ok(PassingBatch { requested_start: start_index, passings }))
        }
        _ => passing_get_error(start_index, response),
    }
}

pub(crate) fn passing_get_from_response_fw26(
    start_index: u32,
    response: CommandResponse,
) -> Result<PassingGetResult<Fw26>, Error> {
    match response.return_code {
        0x00 => {
            let count = parse_passing_get_header(start_index, &response)?;
            let mut passings = Vec::new();
            for line in response.data_lines.iter().skip(1) {
                passings.push(PassingFw26::from_line(line)?);
            }
            if passings.len() != count as usize {
                return Err(Error::Protocol(format!(
                    "PASSINGGET count mismatch: header={count}, records={}",
                    passings.len()
                )));
            }
            Ok(PassingGetResult::Ok(PassingBatch { requested_start: start_index, passings }))
        }
        _ => match passing_get_error(start_index, response)? {
            PassingGetResult::StartIndexNotFound { min_start_index } => {
                Ok(PassingGetResult::StartIndexNotFound { min_start_index })
            }
            PassingGetResult::WrongMode => Ok(PassingGetResult::WrongMode),
            PassingGetResult::Ok(_) => unreachable!(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_25_beaconget() {
        let lines = ["039;1;10;17;64;7;1;1;31;245d;00000000;03;00;00;21;00;00"];
        for line in lines {
            BeaconRecordFw25::from_line(line).unwrap();
        }
    }

    #[test]
    fn test_26_beaconget() {
        let lines = [
            "06f6;0;03;25;32;5;2;0;7b;11fd;095fe75000;03;02;00;00;99;24;00;1a;14;21;+20;0;00;00;00",
            "9dd0;1;00;05;1e;5;0;0;a9;00b5;095fe72300;03;0a;00;00;bf;2d;00;1a;50;81;+26;0;00;00;00",
            "3039;1;10;15;64;5;1;1;31;255e;095fe72302;03;06;00;20;00;00;00;1a;1e;35;+1d;0;04;00;00",
        ];
        for line in lines {
            BeaconRecordFw26::from_line(line).unwrap();
        }
    }

    #[test]
    fn parse_passinginfo_empty_buffer() {
        let info = PassingInfo::from_line("0000;00000000;00000000;00000000;00000000").unwrap();
        assert_eq!(info.count, 0);
        assert_eq!(info.start_id, 0);
        assert_eq!(info.start_timestamp, 0);
        assert_eq!(info.last_id, 0);
        assert_eq!(info.last_timestamp, 0);
    }

    /// `PASSINGINFOGET` example from the command reference (2 passings in buffer).
    #[test]
    fn parse_passinginfo_doc_example() {
        let info = PassingInfo::from_line("0002;00000000;002b6efc;00000001;00823aa8").unwrap();
        assert_eq!(info.count, 2);
        assert_eq!(info.start_id, 0);
        assert_eq!(info.start_timestamp, 0x002b_6efc);
        assert_eq!(info.last_id, 1);
        assert_eq!(info.last_timestamp, 0x0082_3aa8);
    }
}
