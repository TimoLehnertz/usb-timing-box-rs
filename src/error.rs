use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("serial port error: {0}")]
    SerialPort(#[from] serialport::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("command {command} failed with return code {return_code:02x} and data {data_lines:?}")]
    CommandFailed { command: String, return_code: u8, data_lines: Vec<String> },
}
