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
//! # Getting started:
//! ```rust,no_run
//! use race_result_decoder::UsbTimingBox;
//! use std::time::Duration;
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
//! ## Minimum Supported Rust Version (MSRV)
//! The MSRV is 1.88.0 (The version that stabilized let chains).

pub mod commands;
pub mod config_parameter;
pub mod error;
pub mod passing;
pub mod usb_timing_box;
mod utils;

pub use config_parameter::ConfigParameter;
pub use error::Error;
pub use usb_timing_box::*;

pub const DEFAULT_BAUD_RATE: u32 = 19_200;
