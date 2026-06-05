use std::{io, sync::Arc};

#[derive(Debug, Clone, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum Error {
    #[error("io error: {0}")]
    Io(Arc<io::Error>),
    #[error("serial port error: {0}")]
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
