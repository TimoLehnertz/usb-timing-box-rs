//! Firmware format markers for [`crate::UsbTimingBox`] type state.

/// USB Timing Box reporting firmware 2.5 data format (default).
///
/// Timestamps use 32 bits at 256 ticks per second. Startup offset is 24 hours of ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Fw25;

/// USB Timing Box reporting firmware 2.6 data format.
///
/// Enabled via [`crate::ConfigParameter::EnableFw26DataFormat`] / [`crate::UsbTimingBox::enable_fw26_data_format`].
/// Timestamps use 40 bits at 2048 ticks per second with a separate day-adjustment field.
/// Beacon records include signed temperature and additional status fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Fw26;

/// Ticks per second for FW 2.5 passing and epoch timestamps.
pub const FW25_TICKS_PER_SECOND: u32 = 256;

/// Ticks per second for FW 2.6 passing and epoch timestamps.
pub const FW26_TICKS_PER_SECOND: u32 = 2048;

/// Internal timestamp value after box startup (FW 2.5): 24h at 256 Hz.
pub const FW25_TIMESTAMP_INIT_TICKS: u32 = 24 * 3600 * FW25_TICKS_PER_SECOND;

/// Internal timestamp value after box startup (FW 2.6): 7 days at 2048 Hz.
pub const FW26_TIMESTAMP_INIT_TICKS: u64 = 7 * 24 * 3600 * 2048;

/// Ticks per day at 2048 Hz.
pub const FW26_TICKS_PER_DAY: u64 = 24 * 3600 * 2048;
