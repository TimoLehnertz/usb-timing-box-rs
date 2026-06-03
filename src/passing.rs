use crate::{
    error::Error,
    firmware::{Fw25, Fw26},
    utils::{parse_hex_u8, parse_hex_u16, parse_hex_u32, parse_hex_u64, parse_signed_hex_i8},
};
use core::fmt;

/// Combined loop + 2.4 GHz strength byte (FW 2.5+), encoded as `XGGGLLLL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schemars", schemars(transparent))]
pub struct StrengthCombined(pub u8);

impl StrengthCombined {
    /// 2.4 GHz RSSI in dBm: `-90 + 4 * GGG` where `GGG` are bits 4–6.
    pub fn rssi_dbm(self) -> i16 {
        -90 + 4 * i16::from((self.0 >> 4) & 0x07)
    }

    /// 125 kHz loop strength as positive dB (1..=16).
    pub fn loop_strength_db(self) -> u8 {
        1 + (self.0 & 0x0f)
    }
}

/// Parsed passing record (FW 2.5 format).
///
/// Field layout per the reference Python SDK (`ParsePassing`):
/// `[TransponderID];[WakeupCounter:4];[TimeStamp:8];[Hits:2];[Strength:2];[Battery:2];[Temperature:2];
/// [LoopOnly:1];[LoopID:1];[ChannelID:1];[Status:2];[InternalData:2]`
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PassingFw25 {
    pub transponder_id: String,
    /// Transponder wakeup counter for this passing.
    pub wakeup_counter: u16,
    /// Box timestamp in 256th-second ticks (32-bit).
    pub timestamp_ticks: u32,
    pub hits: u8,
    pub strength: StrengthCombined,
    /// Raw battery field from protocol.
    pub battery_raw: u8,
    /// Raw temperature field from protocol.
    pub temperature_raw: u8,
    pub loop_only: bool,
    /// Protocol loop id (0..=7); add 1 for display.
    pub loop_id: u8,
    /// Protocol channel id (0..=7); add 1 for display.
    pub channel_id: u8,
    /// Status / flags field (hex string preserved).
    pub status: String,
    pub internal_data: String,
}

impl PassingFw25 {
    pub fn from_line(line: &str) -> Result<Self, Error> {
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() != 12 {
            return Err(Error::Protocol(format!(
                "FW2.5 passing expected 12 fields, got {} (line: {line})",
                fields.len()
            )));
        }
        Ok(Self {
            transponder_id: fields[0].to_string(),
            wakeup_counter: parse_hex_u16(fields[1])?,
            timestamp_ticks: parse_hex_u32(fields[2])?,
            hits: parse_hex_u8(fields[3])?,
            strength: StrengthCombined(parse_hex_u8(fields[4])?),
            battery_raw: parse_hex_u8(fields[5])?,
            temperature_raw: parse_hex_u8(fields[6])?,
            loop_only: parse_hex_u8(fields[7])? != 0,
            loop_id: parse_hex_u8(fields[8])?,
            channel_id: parse_hex_u8(fields[9])?,
            status: fields[10].to_string(),
            internal_data: fields[11].to_string(),
        })
    }

    pub fn display_loop_id(&self) -> u8 {
        self.loop_id.saturating_add(1)
    }

    pub fn display_channel_id(&self) -> u8 {
        self.channel_id.saturating_add(1)
    }

    /// Converts box ticks to seconds since [`crate::EpochReferenceFw25::unix_time_seconds`].
    pub fn time_seconds_since_epoch(&self, epoch: &crate::EpochReferenceFw25) -> f64 {
        epoch.passing_time_seconds(self.timestamp_ticks)
    }

    /// UTC wall-clock time for this passing (requires the `chrono` feature).
    #[cfg(feature = "chrono")]
    pub fn datetime_utc(&self, epoch: &crate::EpochReferenceFw25) -> Result<chrono::DateTime<chrono::Utc>, Error> {
        utc_datetime_from_seconds(self.time_seconds_since_epoch(epoch))
    }
}

impl fmt::Display for PassingFw25 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{};{:04x};{:08x};{:02x};{:02x};{:02x};{:02x};{};{:x};{:x};{};{}",
            self.transponder_id,
            self.wakeup_counter,
            self.timestamp_ticks,
            self.hits,
            self.strength.0,
            self.battery_raw,
            self.temperature_raw,
            u8::from(self.loop_only),
            self.loop_id,
            self.channel_id,
            self.status,
            self.internal_data
        )
    }
}

/// Parsed passing record (FW 2.6 format, after `CONFSET;B3;1`).
///
/// Documented layout (14 fields, or 15 when a transponder reports an extra code):
/// `[TransponderID];[WakeupCounter:4];[TimeStamp:10];[Hits:2];[Strength:2];[Battery:2];[Temperature:+2];
/// [LoopOnly:1];[LoopID:1];[ChannelID:1];[Status:2];[InternalData:2];[RFU:2];[Extra:2]` and optionally
/// `;[ExtraCode]` (e.g. `A-1000`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PassingFw26 {
    pub transponder_id: String,
    pub wakeup_counter: u16,
    /// Box timestamp in 2048th-second ticks (40-bit, 10 hex digits, includes day adjustment).
    pub timestamp_ticks: u64,
    pub hits: u8,
    pub strength: StrengthCombined,
    pub battery_raw: u8,
    /// Signed temperature in °C (`+15`, `+1a`, …).
    pub temperature_celsius: i8,
    pub loop_only: bool,
    pub loop_id: u8,
    pub channel_id: u8,
    pub status: String,
    pub internal_data: String,
    /// Reserved field (`RFU`), typically `00`.
    pub rfu: String,
    /// Extra field before optional [`Self::extra_code`] (e.g. `1` or `0`).
    pub extra: String,
    /// Optional transponder extra code (15-field records only), e.g. `A-1000`.
    pub extra_code: Option<String>,
}

impl PassingFw26 {
    /// Parses a passing line, stripping an optional ASCII checksum suffix (`=XX`).
    pub fn from_line(line: &str) -> Result<Self, Error> {
        Self::from_fields(strip_line_checksum(line).split(';').map(str::trim).collect())
    }

    fn from_fields(fields: Vec<&str>) -> Result<Self, Error> {
        match fields.len() {
            14 => Ok(Self {
                transponder_id: fields[0].to_string(),
                wakeup_counter: parse_hex_u16(fields[1])?,
                timestamp_ticks: parse_hex_u64(fields[2])?,
                hits: parse_hex_u8(fields[3])?,
                strength: StrengthCombined(parse_hex_u8(fields[4])?),
                battery_raw: parse_hex_u8(fields[5])?,
                temperature_celsius: parse_signed_hex_i8(fields[6])?,
                loop_only: parse_hex_u8(fields[7])? != 0,
                loop_id: parse_hex_u8(fields[8])?,
                channel_id: parse_hex_u8(fields[9])?,
                status: fields[10].to_string(),
                internal_data: fields[11].to_string(),
                rfu: fields[12].to_string(),
                extra: fields[13].to_string(),
                extra_code: None,
            }),
            15 => Ok(Self {
                transponder_id: fields[0].to_string(),
                wakeup_counter: parse_hex_u16(fields[1])?,
                timestamp_ticks: parse_hex_u64(fields[2])?,
                hits: parse_hex_u8(fields[3])?,
                strength: StrengthCombined(parse_hex_u8(fields[4])?),
                battery_raw: parse_hex_u8(fields[5])?,
                temperature_celsius: parse_signed_hex_i8(fields[6])?,
                loop_only: parse_hex_u8(fields[7])? != 0,
                loop_id: parse_hex_u8(fields[8])?,
                channel_id: parse_hex_u8(fields[9])?,
                status: fields[10].to_string(),
                internal_data: fields[11].to_string(),
                rfu: fields[12].to_string(),
                extra: fields[13].to_string(),
                extra_code: Some(fields[14].to_string()),
            }),
            n => Err(Error::Protocol(format!(
                "FW2.6 passing expected 14 or 15 fields, got {n} (line: {})",
                fields.join(";")
            ))),
        }
    }

    pub fn display_loop_id(&self) -> u8 {
        self.loop_id.saturating_add(1)
    }

    pub fn display_channel_id(&self) -> u8 {
        self.channel_id.saturating_add(1)
    }

    pub fn time_seconds_since_epoch(&self, epoch: crate::EpochReferenceFw26) -> f64 {
        epoch.passing_time_seconds(self.timestamp_ticks)
    }

    /// UTC wall-clock time for this passing (requires the `chrono` feature).
    #[cfg(feature = "chrono")]
    pub fn datetime_utc(&self, epoch: crate::EpochReferenceFw26) -> Result<chrono::DateTime<chrono::Utc>, Error> {
        utc_datetime_from_seconds(self.time_seconds_since_epoch(epoch))
    }
}

impl fmt::Display for PassingFw26 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{};{:04x};{:010x};{:02x};{:02x};{:02x};{:+02x};{};{:x};{:x};{};{};{};{}",
            self.transponder_id,
            self.wakeup_counter,
            self.timestamp_ticks,
            self.hits,
            self.strength.0,
            self.battery_raw,
            self.temperature_celsius,
            u8::from(self.loop_only),
            self.loop_id,
            self.channel_id,
            self.status,
            self.internal_data,
            self.rfu,
            self.extra
        )?;
        if let Some(code) = &self.extra_code {
            write!(f, ";{code}")?;
        }
        Ok(())
    }
}

/// Strips an optional per-line checksum suffix (`=XX`) before parsing.
pub(crate) fn strip_line_checksum(line: &str) -> &str {
    line.split('=').next().unwrap_or(line).trim()
}

#[cfg(feature = "chrono")]
fn utc_datetime_from_seconds(seconds: f64) -> Result<chrono::DateTime<chrono::Utc>, Error> {
    use chrono::TimeZone;
    let secs = seconds.trunc() as i64;
    let nanos = ((seconds - secs as f64) * 1e9).round() as u32;
    chrono::Utc
        .timestamp_opt(secs, nanos)
        .single()
        .ok_or_else(|| Error::Protocol(format!("passing timestamp out of range: {seconds}")))
}

/// Passing batch returned by [`crate::PassingGetResult`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "serde",
    serde(bound(serialize = "PassingOf<F>: serde::Serialize", deserialize = "PassingOf<F>: serde::Deserialize<'de>"))
)]
#[cfg_attr(feature = "schemars", schemars(bound = "PassingOf<F>: schemars::JsonSchema"))]
pub struct PassingBatch<F: FirmwarePassing> {
    pub requested_start: u32,
    pub passings: Vec<PassingOf<F>>,
}

/// Resolves the passing type for firmware marker `F`.
pub type PassingOf<F> = <F as FirmwarePassing>::Passing;

/// Optional [`serde`] bounds (no-op when the feature is disabled).
#[cfg(feature = "serde")]
mod passing_serde {
    pub trait Bound: serde::Serialize + serde::de::DeserializeOwned {}
    impl<T> Bound for T where T: serde::Serialize + serde::de::DeserializeOwned {}
}
#[cfg(not(feature = "serde"))]
mod passing_serde {
    pub trait Bound {}
    impl<T> Bound for T {}
}

/// Optional [`schemars`] bounds (no-op when the feature is disabled).
#[cfg(feature = "schemars")]
mod passing_schemars {
    pub trait Bound: schemars::JsonSchema {}
    impl<T> Bound for T where T: schemars::JsonSchema {}
}
#[cfg(not(feature = "schemars"))]
mod passing_schemars {
    pub trait Bound {}
    impl<T> Bound for T {}
}

/// Associates a [`PassingFw25`] or [`PassingFw26`] type with a firmware marker.
pub trait FirmwarePassing: passing_schemars::Bound {
    type Passing: Clone + fmt::Debug + PartialEq + Eq + passing_serde::Bound + passing_schemars::Bound;
}

impl FirmwarePassing for Fw25 {
    type Passing = PassingFw25;
}

impl FirmwarePassing for Fw26 {
    type Passing = PassingFw26;
}

impl<F: FirmwarePassing> PassingBatch<F> {
    pub fn next_start_index(&self) -> u32 {
        self.requested_start + self.passings.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quick-start example (`GLBAS60;…`).
    #[test]
    fn parse_fw25_quick_start_example() {
        let p = PassingFw25::from_line("GLBAS60;0718;01521527;0c;08;9f;1a;0;1;2;00;0").unwrap();
        assert_eq!(p.transponder_id, "GLBAS60");
        assert_eq!(p.wakeup_counter, 0x0718);
        assert_eq!(p.timestamp_ticks, 0x0152_1527);
        assert_eq!(p.hits, 0x0c);
        assert_eq!(p.strength.0, 0x08);
    }

    /// `PASSINGGET` standard format example from the command reference.
    #[test]
    fn parse_fw25_doc_passingget_standard() {
        let p = PassingFw25::from_line("IKNWZ06;a153;093a9eb4;fe;71;1d;15;0;1;7;00;0").unwrap();
        assert_eq!(p.transponder_id, "IKNWZ06");
        assert_eq!(p.wakeup_counter, 0xa153);
        assert_eq!(p.timestamp_ticks, 0x093a_9eb4);
        assert_eq!(p.hits, 0xfe);
        assert_eq!(p.strength.0, 0x71);
        assert_eq!(p.battery_raw, 0x1d);
        assert_eq!(p.temperature_raw, 0x15);
        assert!(!p.loop_only);
        assert_eq!(p.loop_id, 1);
        assert_eq!(p.channel_id, 7);
        assert_eq!(p.display_loop_id(), 2);
        assert_eq!(p.display_channel_id(), 8);
        assert_eq!(p.status, "00");
        assert_eq!(p.internal_data, "0");
    }

    /// FW 2.6 `PASSINGGET` example (14 fields, extra code in field 13).
    #[test]
    fn parse_fw26_doc_passingget_single() {
        let p = PassingFw26::from_line("IKNWZ06;a153;093a9eb470;fe;71;1d;+15;0;1;7;00;0;00;1").unwrap();
        assert_eq!(p.transponder_id, "IKNWZ06");
        assert_eq!(p.wakeup_counter, 0xa153);
        assert_eq!(p.timestamp_ticks, 0x093a_9eb470);
        assert_eq!(p.hits, 0xfe);
        assert_eq!(p.strength.0, 0x71);
        assert_eq!(p.battery_raw, 0x1d);
        assert_eq!(p.temperature_celsius, 0x15);
        assert_eq!(p.loop_id, 1);
        assert_eq!(p.channel_id, 7);
        assert_eq!(p.status, "00");
        assert_eq!(p.internal_data, "0");
        assert_eq!(p.rfu, "00");
        assert_eq!(p.extra, "1");
        assert_eq!(p.extra_code, None);
    }

    /// FW 2.6 example with two passings; second transponder reports extra code `A-1000`.
    #[test]
    fn parse_fw26_doc_passingget_extra_code() {
        let second = PassingFw26::from_line("ZILAL95;9087;093ab54213;fe;71;1d;+1a;0;1;7;00;0;00;0;A-1000").unwrap();
        assert_eq!(second.transponder_id, "ZILAL95");
        assert_eq!(second.wakeup_counter, 0x9087);
        assert_eq!(second.timestamp_ticks, 0x093a_b54213);
        assert_eq!(second.temperature_celsius, 0x1a);
        assert_eq!(second.extra, "0");
        assert_eq!(second.extra_code.as_deref(), Some("A-1000"));
    }

    /// FW 2.6 batch example with checksum suffixes (`=XX`) on each line.
    #[test]
    fn parse_fw26_doc_passingget_with_checksum() {
        let lines = [
            "IKNWZ06;a153;093a9eb470;fe;71;1d;+15;0;1;7;00;0;00;1=85",
            "ZILAL95;9087;093ab54210;fe;71;1d;+1a;0;1;7;00;0;00;0=42",
            "ZILAL95;908e;093ad85370;fe;71;1d;+1a;0;1;7;00;0;00;0=7D",
            "ZILAL95;915c;093ee28f50;fe;71;1d;+1a;0;1;7;00;0;00;0=AC",
        ];

        for line in lines {
            PassingFw26::from_line(line).unwrap();
        }
    }

    #[test]
    fn parse_fw26_doc_passingget_all_eight_checksum_lines() {
        let lines = [
            "IKNWZ06;a153;093a9eb470;fe;71;1d;+15;0;1;7;00;0;00;1=85",
            "ZILAL95;9087;093ab54210;fe;71;1d;+1a;0;1;7;00;0;00;0=42",
            "ZILAL95;908e;093ad85370;fe;71;1d;+1a;0;1;7;00;0;00;0=7D",
            "ZILAL95;9093;093af16d30;fe;71;1d;+1a;0;1;7;00;0;00;0=75",
            "ZILAL95;90a9;093b5fe320;fe;71;1d;+1a;0;1;7;00;0;00;0=A5",
            "ZILAL95;90b6;093ba10a00;fe;71;1d;+1a;0;1;7;00;0;00;0=91",
            "ZILAL95;9144;093e6a2150;fe;71;1d;+1a;0;1;7;00;0;00;0=41",
            "ZILAL95;915c;093ee28f50;fe;71;1d;+1a;0;1;7;00;0;00;0=AC",
        ];
        for line in lines {
            PassingFw26::from_line(line).unwrap();
        }
    }

    #[test]
    fn fw25_passing() {
        let passing = PassingFw25::from_line("ZBAAA03;04c6;002b6efc;11;19;1d;15;0;1;1;00;0").unwrap();
        assert_eq!(passing.transponder_id, "ZBAAA03");
    }
}
