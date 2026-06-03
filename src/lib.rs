//! Rust library for interfacing with a race result usb timing box via serial connection.
//!
//! # Disclaimer
//!
//! This project is not associated with race result in any way.
//!
//! # Documentation
//! This crate is based on the official documentation for the USB Timing Box. You can find
//! that [here](https://www.raceresult.com/en/shophelp/index?id=346-Commands) and
//! [here](https://www.raceresult.com/en/support/kbexport2?id=18).
//! Most data structures and functions inside this crate are documented with
//! relevant snippets from the official documentation. However it is highly recommended
//! to read the official documentation for the most up to date information.
//!
//! # Firmware data format (type state)
//!
//! [`UsbTimingBox`] is parameterized by [`Fw25`] (default) or [`Fw26`]. Call
//! [`UsbTimingBox::enable_fw26_data_format`] on a FW 2.5 client to switch the device and
//! obtain a [`UsbTimingBox<Fw26>`] that parses passings, beacons, and timestamps accordingly.
//!
//! # Getting started:
//! ```rust,no_run
//! # use usb_timing_box_rs::UsbTimingBox;
//! # use std::time::Duration;
//!
//! let mut box_client = UsbTimingBox::builder("COM3").connect().unwrap();
//!
//! box_client.wait_for_autoboot(Duration::from_secs(4)).unwrap();
//! box_client.switch_to_ascii_protocol().unwrap();
//!
//! let site_survey = box_client.site_survey().unwrap();
//! println!("Site survey: {:?}", site_survey);
//! ```
//!
//! Please also take a look at the example.
//!
//! # Features
//!
//! Optional Cargo features extend the passing types ([`PassingFw25`], [`PassingFw26`],
//! [`StrengthCombined`], [`PassingBatch`], and firmware markers [`Fw25`] / [`Fw26`]):
//!
//! - **`serde`** — [`Serialize`](https://docs.rs/serde/latest/serde/trait.Serialize.html) and
//!   [`Deserialize`](https://docs.rs/serde/latest/serde/trait.Deserialize.html)
//! - **`schemars`** — [`JsonSchema`](https://docs.rs/schemars/latest/schemars/trait.JsonSchema.html)
//!   (e.g. `schemars::schema_for!(PassingFw25)`)
//! - **`chrono`** — [`PassingFw25::datetime_utc`] / [`PassingFw26::datetime_utc`] returning
//!   [`chrono::DateTime`](https://docs.rs/chrono/latest/chrono/struct.DateTime.html) in UTC
//!
//! # Minimum Supported Rust Version (MSRV)
//! The MSRV is 1.88.0 (The version that stabilized let chains).

pub mod commands;
pub mod config_parameter;
pub mod error;
pub mod firmware;
pub mod passing;
pub mod usb_timing_box;
mod utils;

pub use commands::{
    BeaconMode, BeaconPowerStatus, BeaconRecordFw25, BeaconRecordFw26, BoxType, CommandResponse, EpochReferenceFw25,
    EpochReferenceFw26, LoopStatus, OperationMode, PassingGetResult, PassingInfo, PowerConnection,
};
pub use config_parameter::ConfigParameter;
pub use error::Error;
pub use firmware::{FW25_TICKS_PER_SECOND, FW26_TICKS_PER_SECOND, Fw25, Fw26};
pub use passing::{PassingBatch, PassingFw25, PassingFw26, StrengthCombined};
pub use usb_timing_box::*;

pub const DEFAULT_BAUD_RATE: u32 = 19_200;
