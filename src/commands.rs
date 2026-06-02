use crate::{
    Error,
    passing::PassingBatch,
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
pub struct PassingInfo {
    /// Number of passings in buffer.
    pub count: u16,
    /// Index of first passing in buffer.
    pub start_id: u32,
    /// Timestamp of first passing in buffer.
    pub start_timestamp: u32,
    /// Index of last passing in buffer.
    pub last_id: u32,
    /// Timestamp of last passing in buffer.
    pub last_timestamp: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassingGetResult {
    Ok(PassingBatch),

    /// In case of this error, you need to execute `PASSINGGET;[MinStartIndex]` again.
    StartIndexTooLow {
        /// indicating the lowest StartIndex that is available in memory
        min_start_index: u32,
    },

    /// The Box is running in a Mode which does not allow pulling passings from it.
    /// Most likely this is a USB Timing Box in Repeat Mode. The User needs to switch mode.
    WrongMode,
}

/// Parsed beacon record returned by [`UsbTimingBox::beacon_get`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaconRecord {
    /// Device ID of system (4 hex digits in protocol).
    pub active_device_id: u16,
    /// Loop status reported by the box.
    pub loop_status: LoopStatus,
    /// Beacon mode / box mode marker.
    pub mode: BeaconMode,
    /// Reserved field in docs ("LoopData"), kept for completeness.
    pub loop_data: u8,
    /// Current loop power setting (raw protocol value, 0x00..0xff).
    pub loop_power: u8,
    /// Channel ID in protocol numbering (0..f maps to displayed 1..8).
    pub channel_id: u8,
    /// Loop ID in protocol numbering (0..f maps to displayed 1..8).
    pub loop_id: u8,
    /// Current power source.
    pub power_connection: PowerConnection,
    /// Raw power status value from beacon.
    pub power_status_raw: u8,
    /// Beacon index since startup.
    pub beacon_index: u16,
    /// Internal box time ticks (32-bit/256Hz in standard, 40-bit/2048Hz in FW2.6).
    pub time_ticks: u64,
    /// Average channel noise level (0..10 where 0 = no noise).
    pub channel_noise_avg: u8,
    /// Average transponder energy detect value (0..85).
    pub transponder_energy_detect: u8,
    /// Average beacon energy detect value (0..85).
    pub beacon_energy_detect: u8,
    /// Extra FW2.6 fields, if present.
    pub extended: Option<BeaconExtended>,
}

impl BeaconRecord {
    pub fn from_line(line: &str) -> Result<Self, Error> {
        let fields: Vec<&str> = line.split(';').collect();
        match fields.len() {
            17 => Self::parse_standard(&fields),
            26 => Self::parse_fw26(&fields),
            n => Err(Error::Protocol(format!("BEACONGET beacon has unexpected field count {n} (line: {line})"))),
        }
    }

    fn parse_standard(fields: &[&str]) -> Result<Self, Error> {
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
            channel_noise_avg: parse_hex_u8(fields[12])?,
            transponder_energy_detect: parse_hex_u8(fields[14])?,
            beacon_energy_detect: parse_hex_u8(fields[16])?,
            extended: None,
        })
    }

    fn parse_fw26(fields: &[&str]) -> Result<Self, Error> {
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
            channel_noise_avg: parse_hex_u8(fields[12])?,
            transponder_energy_detect: parse_hex_u8(fields[14])?,
            beacon_energy_detect: parse_hex_u8(fields[16])?,
            extended: Some(BeaconExtended {
                beacon_version: parse_hex_u8(fields[11])?,
                beacon_success_rate: parse_hex_u8(fields[17])?,
                fw_version_raw: parse_hex_u8(fields[18])?,
                box_type: BoxType::from_raw(parse_hex_u8(fields[19])?),
                box_mode_raw: parse_hex_u8(fields[20])?,
                temperature_celsius: parse_signed_hex_i8(fields[21])?,
                buffer_overflow: parse_hex_u8(fields[22])? != 0,
                buffer_fill_state_percent: parse_hex_u8(fields[23])?,
                avg_transponder_retries: parse_hex_u8(fields[24])?,
                avg_repeat_retries: parse_hex_u8(fields[25])?,
            }),
        })
    }

    /// Returns display channel number (`protocol channel + 1`).
    pub fn display_channel(&self) -> u8 {
        self.channel_id.saturating_add(1)
    }

    /// Returns display loop id (`protocol loop id + 1`).
    pub fn display_loop_id(&self) -> u8 {
        self.loop_id.saturating_add(1)
    }

    /// Returns transponder RSSI estimate in dBm (`-90 + value`).
    pub fn transponder_rssi_dbm(&self) -> i16 {
        -90 + i16::from(self.transponder_energy_detect)
    }

    /// Returns beacon/base RSSI estimate in dBm (`-90 + value`).
    pub fn beacon_rssi_dbm(&self) -> i16 {
        -90 + i16::from(self.beacon_energy_detect)
    }

    /// Interprets power status as hours when on battery, else as volts.
    pub fn power_status(&self) -> BeaconPowerStatus {
        match self.power_connection {
            PowerConnection::Battery => BeaconPowerStatus::BatteryHours(self.power_status_raw),
            _ => BeaconPowerStatus::Voltage(self.power_status_raw as f32 / 10.0),
        }
    }
}

/// Additional fields present in FW2.6 beacon format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaconExtended {
    pub beacon_version: u8,
    /// 0..10 where 10 means all expected beacons received.
    pub beacon_success_rate: u8,
    /// Raw version value where `26` represents `2.6`.
    pub fw_version_raw: u8,
    pub box_type: BoxType,
    /// Raw mode value; interpretation depends on device type.
    pub box_mode_raw: u8,
    pub temperature_celsius: i8,
    pub buffer_overflow: bool,
    /// 0..100 where 100 means full.
    pub buffer_fill_state_percent: u8,
    pub avg_transponder_retries: u8,
    pub avg_repeat_retries: u8,
}

impl BeaconExtended {
    /// Converts raw FW version encoding (`26` => `2.6`) to a float.
    pub fn fw_version(&self) -> f32 {
        self.fw_version_raw as f32 / 10.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BeaconPowerStatus {
    BatteryHours(u8),
    Voltage(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopStatus {
    Ok,
    Fault,
    Limit,
    OverVoltage,
    Unknown(u8),
}

impl LoopStatus {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0x00 => Self::Ok,
            0x01 => Self::Fault,
            0x02 => Self::Limit,
            0x04 => Self::OverVoltage,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub enum BoxType {
    ActiveExtension,
    LoopBox,
    ManagementBox,
    UsbTimingBox,
    Ubidium,
    Unknown(u8),
}

impl BoxType {
    fn from_raw(raw: u8) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochReference {
    pub unix_time_seconds: u32,
    /// The USB Timing Box operates on a 0.28ppm time base, counting 256 ticks per second.
    pub timestamp_ticks: u32,
}

impl EpochReference {
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

    pub fn passing_time_seconds(self, passing_timestamp_ticks: u32) -> f64 {
        let delta = passing_timestamp_ticks.wrapping_sub(self.timestamp_ticks) as f64;
        self.unix_time_seconds as f64 + (delta / 256.0)
    }
}
