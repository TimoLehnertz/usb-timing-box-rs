use std::{io, sync::Arc};

#[derive(Debug, Clone, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(content = "data", tag = "type"))]
#[cfg_attr(feature = "serde", serde(rename_all_fields = "camelCase"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum Error {
    #[error("io error: {0}")]
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_debug"))]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    Io(Arc<io::Error>),
    #[error("serial port error: {0}")]
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_debug"))]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    SerialPort(#[from] serialport::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("command {command} failed with return code {return_code:02x} and data {data_lines:?}")]
    CommandFailed { command: String, return_code: u8, data_lines: Vec<String> },
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(Arc::new(error))
    }
}

#[cfg(feature = "serde")]
fn serialize_debug<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: std::fmt::Debug,
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{value:?}"))
}
