use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

use crate::{
    ConfigParameter, DEFAULT_BAUD_RATE,
    commands::{BeaconRecord, CommandResponse, EpochReference, OperationMode, PassingGetResult, PassingInfo},
    error::Error,
    passing::{Passing, PassingBatch},
    utils::{hex2, hex8, parse_hex_u8, parse_hex_u16, parse_hex_u32, unix_time_now},
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct UsbTimingBoxBuilder {
    port_name: String,
    baud_rate: u32,
    timeout: Duration,
    dtr_low_on_open: bool,
    rts_low_on_open: bool,
}

impl UsbTimingBoxBuilder {
    pub fn new(port_name: impl Into<String>) -> Self {
        Self {
            port_name: port_name.into(),
            baud_rate: DEFAULT_BAUD_RATE,
            timeout: Duration::from_millis(200),
            dtr_low_on_open: true,
            rts_low_on_open: true,
        }
    }

    pub fn baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn dtr_low_on_open(mut self, dtr_low_on_open: bool) -> Self {
        self.dtr_low_on_open = dtr_low_on_open;
        self
    }

    pub fn rts_low_on_open(mut self, rts_low_on_open: bool) -> Self {
        self.rts_low_on_open = rts_low_on_open;
        self
    }

    pub fn connect(self) -> Result<UsbTimingBox, Error> {
        let mut port = serialport::new(self.port_name, self.baud_rate)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .flow_control(FlowControl::None)
            .timeout(self.timeout)
            .open()?;

        if self.dtr_low_on_open {
            port.write_data_terminal_ready(false)?;
        }
        if self.rts_low_on_open {
            port.write_request_to_send(false)?;
        }

        Ok(UsbTimingBox { port, scratch: Vec::new() })
    }
}

#[derive(Debug)]
pub struct UsbTimingBox {
    port: Box<dyn SerialPort>,
    scratch: Vec<u8>,
}

impl UsbTimingBox {
    pub fn builder(port_name: impl Into<String>) -> UsbTimingBoxBuilder {
        UsbTimingBoxBuilder::new(port_name)
    }

    pub fn connect(port_name: impl Into<String>) -> Result<Self, Error> {
        UsbTimingBoxBuilder::new(port_name).connect()
    }

    pub fn set_mode(&mut self, mode: OperationMode) -> Result<(), Error> {
        self.conf_set(ConfigParameter::OperationMode, mode as u8)
    }

    pub fn set_channel_id(&mut self, channel: u8) -> Result<(), Error> {
        if channel > 0x07 {
            return Err(Error::Protocol(format!("channel out of range: {channel}")));
        }
        self.conf_set(ConfigParameter::ChannelId, channel)
    }

    pub fn read_channel_id(&mut self) -> Result<u8, Error> {
        self.conf_get(ConfigParameter::ChannelId)
    }

    pub fn set_loop_id(&mut self, loop_id: u8) -> Result<(), Error> {
        if loop_id > 0x07 {
            return Err(Error::Protocol(format!("loop id out of range: {loop_id}")));
        }
        self.conf_set(ConfigParameter::LoopId, loop_id)
    }

    pub fn read_loop_id(&mut self) -> Result<u8, Error> {
        self.conf_get(ConfigParameter::LoopId)
    }

    pub fn set_loop_power_percent(&mut self, power_percent: u8) -> Result<(), Error> {
        if power_percent > 100 {
            return Err(Error::Protocol(format!("loop power out of range: {power_percent}")));
        }
        self.conf_set(ConfigParameter::LoopPower, power_percent)
    }

    pub fn read_loop_power_percent(&mut self) -> Result<u8, Error> {
        self.conf_get(ConfigParameter::LoopPower)
    }

    pub fn port_mut(&mut self) -> &mut dyn SerialPort {
        &mut *self.port
    }

    pub fn set_dtr(&mut self, value: bool) -> Result<(), Error> {
        self.port.write_data_terminal_ready(value)?;
        Ok(())
    }

    pub fn set_rts(&mut self, value: bool) -> Result<(), Error> {
        self.port.write_request_to_send(value)?;
        Ok(())
    }

    pub fn pulse_dtr_high(&mut self, duration: Duration) -> Result<(), Error> {
        self.set_dtr(true)?;
        std::thread::sleep(duration);
        self.set_dtr(false)?;
        Ok(())
    }

    /// From the docs:
    /// On power up, the USB Timing Box goes through a bootloader process.
    /// For 3 seconds after power up, the bootloader is waiting for instructions to upgrade the firmware.
    /// During these 3 seconds any character sent to the box will stop it from booting!
    /// This results in a "dead" box with LED continuously on.
    ///
    /// Do not send any data to the USB Timing Box within 3 seconds after power up or reset.
    /// It is best practice to wait 3s or until the USB Timing Box issues
    /// AUTOBOOT\n indicating, that the bootloader is done.
    pub fn wait_for_autoboot(&mut self, timeout: Duration) -> Result<bool, Error> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(line) = self.try_read_line()?
                && line.trim_end() == "AUTOBOOT"
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn command(&mut self, command: &str) -> Result<CommandResponse, Error> {
        self.command_with_args::<&str>(command, &[])
    }

    pub fn command_with_args<S: AsRef<str>>(&mut self, command: &str, args: &[S]) -> Result<CommandResponse, Error> {
        // Calling a Command
        // Commands always start with the uppercase name, which can be followed by parameters
        // which are separated by a semicolon and terminated with a newline \n.
        let mut line = String::from(command);
        for arg in args {
            line.push(';');
            line.push_str(arg.as_ref());
        }
        line.push('\n');
        self.port.write_all(line.as_bytes())?;
        self.port.flush()?;
        self.read_command_response(command)
    }

    pub fn raw_command_line(&mut self, line_without_newline: &str) -> Result<CommandResponse, Error> {
        let mut line = String::from(line_without_newline);
        line.push('\n');
        self.port.write_all(line.as_bytes())?;
        self.port.flush()?;
        let command =
            line_without_newline.split(';').next().ok_or_else(|| Error::Protocol("empty command".to_string()))?;
        self.read_command_response(command)
    }

    /// From the docs:
    /// USB Timing Box FW 2.5 and up automatically switches to ASCII-Timing Protocol.
    /// FW 2.4 and earlier boots up in a "debug interface". On FW 2.4 you are required to
    /// call ASCII\n to switch to the ASCII-Timing Protocol. Be aware that RACE RESULT
    /// does not recommend any use of the debug interface. The behavior of the debug
    /// interface can change without notice. On FW 2.5 and up, you can switch back to
    /// debug interface by issuing the DEBUG\n command.
    ///
    /// # ASCII resets:
    /// - Push Prewarn OFF
    /// - Push Passings OFF
    /// - Check Sum Mode OFF
    pub fn switch_to_ascii_protocol(&mut self) -> Result<(), Error> {
        let response = self.command("ASCII")?;
        self.ensure_code(&response, 0x00)
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        let response = self.command("RESET")?;
        self.ensure_code(&response, 0x00)
    }

    /// **FW 2.5 and up only!**
    ///
    /// Scans all eight 2.4Ghz channels and reports noise values.
    /// These can be used to select a suitable channel.
    /// The 2.4GHz receiver will be blocked for about 2 seconds.
    /// In this period it is not possible to receive any transponder data.
    /// Do not call this command during normal timing operation!
    ///
    /// # Returns
    /// 00 = no noise
    /// 0A(10) = channel 100% blocked by noise
    /// A channel with more than >50% noise is not recommended to be used.
    pub fn site_survey(&mut self) -> Result<[u8; 8], Error> {
        let response = self.command("SITESURVEY")?;
        self.ensure_code(&response, 0x00)?;
        if response.data_lines.len() != 8 {
            return Err(Error::Protocol(format!(
                "SITESURVEY expected 8 data lines, got {}",
                response.data_lines.len()
            )));
        }

        let mut out = [0_u8; 8];
        for line in &response.data_lines {
            let mut parts = line.split(';');

            let channel =
                parts.next().ok_or_else(|| Error::Protocol("missing channel".to_string())).and_then(parse_hex_u8)?;

            let noise =
                parts.next().ok_or_else(|| Error::Protocol("missing noise".to_string())).and_then(parse_hex_u8)?;

            let slot = out
                .get_mut(channel as usize)
                .ok_or_else(|| Error::Protocol(format!("invalid channel in SITESURVEY: {channel}")))?;

            *slot = noise;
        }
        Ok(out)
    }

    pub fn conf_get(&mut self, parameter: ConfigParameter) -> Result<u8, Error> {
        let response = self.command_with_args("CONFGET", &[hex2(parameter.id())])?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("CONFGET missing data line".to_string()))?;
        let mut parts = line.split(';');
        let _id =
            parts.next().ok_or_else(|| Error::Protocol("CONFGET missing id".to_string())).and_then(parse_hex_u8)?;
        let value =
            parts.next().ok_or_else(|| Error::Protocol("CONFGET missing value".to_string())).and_then(parse_hex_u8)?;
        Ok(value)
    }

    /// Sets the parameter parameter to the value.
    pub fn conf_set(&mut self, parameter: ConfigParameter, value: u8) -> Result<(), Error> {
        let response = self.command_with_args("CONFSET", &[hex2(parameter.id()), hex2(value)])?;
        self.ensure_code(&response, 0x00)
    }

    pub fn info_get_raw(&mut self, parameter_id: u8) -> Result<String, Error> {
        let response = self.command_with_args("INFOGET", &[hex2(parameter_id)])?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("INFOGET missing data line".to_string()))?;
        let mut parts = line.split(';');
        let _id =
            parts.next().ok_or_else(|| Error::Protocol("INFOGET missing id".to_string())).and_then(parse_hex_u8)?;
        let value = parts.next().ok_or_else(|| Error::Protocol("INFOGET missing value".to_string()))?;
        Ok(value.to_string())
    }

    pub fn epoch_ref_get(&mut self) -> Result<EpochReference, Error> {
        let response = self.command("EPOCHREFGET")?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFGET missing data line".to_string()))?;

        EpochReference::parse(line)
    }

    pub fn epoch_ref_set(&mut self, epoch_seconds: u32) -> Result<EpochReference, Error> {
        let response = self.command_with_args("EPOCHREFSET", &[hex8(epoch_seconds)])?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFSET missing data line".to_string()))?;

        EpochReference::parse(line)
    }

    pub fn epoch_ref_set_with_dtr_pulse(
        &mut self,
        epoch_seconds: u32,
        pulse_length: Duration,
    ) -> Result<EpochReference, Error> {
        let mut line = String::from("EPOCHREFSET;");
        line.push_str(&hex8(epoch_seconds));
        line.push('\n');
        self.port.write_all(line.as_bytes())?;
        self.port.flush()?;
        self.pulse_dtr_high(pulse_length)?;
        let response = self.read_command_response("EPOCHREFSET")?;
        self.ensure_code(&response, 0x00)?;
        let body =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFSET missing data line".to_string()))?;

        EpochReference::parse(body)
    }

    pub fn epoch_ref_sync_to_next_second(&mut self) -> Result<EpochReference, Error> {
        let now = unix_time_now()?;
        let target = now.saturating_add(1);
        let mut line = String::from("EPOCHREFSET;");
        line.push_str(&hex8(target));
        line.push('\n');
        self.port.write_all(line.as_bytes())?;
        self.port.flush()?;

        while unix_time_now()? < target {
            std::thread::sleep(Duration::from_millis(2));
        }
        self.pulse_dtr_high(Duration::from_millis(200))?;

        let response = self.read_command_response("EPOCHREFSET")?;
        self.ensure_code(&response, 0x00)?;
        let body =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFSET missing data line".to_string()))?;

        EpochReference::parse(body)
    }

    pub fn timestamp_get(&mut self) -> Result<u32, Error> {
        let response = self.command("TIMESTAMPGET")?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("TIMESTAMPGET missing data line".to_string()))?;

        parse_hex_u32(line)
    }

    /// **FW 2.5 and up**
    ///
    /// Reports information about passings in the internal buffer.
    pub fn passing_info_get(&mut self) -> Result<PassingInfo, Error> {
        let response = self.command("PASSINGINFOGET")?;
        self.ensure_code(&response, 0x00)?;
        let line = response
            .single_data_line()
            .ok_or_else(|| Error::Protocol("PASSINGINFOGET missing data line".to_string()))?;

        let mut parts = line.split(';');

        Ok(PassingInfo {
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

    pub fn passing_get(&mut self, start_index: u32) -> Result<PassingGetResult, Error> {
        let response = self.command_with_args("PASSINGGET", &[hex8(start_index)])?;
        match response.return_code {
            0x00 => {
                let header = response
                    .data_lines
                    .first()
                    .ok_or_else(|| Error::Protocol("PASSINGGET missing header line".to_string()))?;

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

                let count = parts
                    .next()
                    .ok_or_else(|| Error::Protocol("PASSINGGET missing count".to_string()))
                    .and_then(parse_hex_u8)?;

                let mut passings = Vec::new();

                for line in response.data_lines.iter().skip(1) {
                    passings.push(Passing::from_line(line)?);
                }

                if passings.len() != count as usize {
                    return Err(Error::Protocol(format!(
                        "PASSINGGET count mismatch: header={count}, records={}",
                        passings.len()
                    )));
                }

                Ok(PassingGetResult::Ok(PassingBatch { requested_start: start_index, passings }))
            }
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

                Ok(PassingGetResult::StartIndexTooLow { min_start_index })
            }

            0x11 => Ok(PassingGetResult::WrongMode),

            other => Err(Error::CommandFailed {
                command: response.command,
                return_code: other,
                data_lines: response.data_lines,
            }),
        }
    }

    /// Queries status beacons from active boxes (`BEACONGET`).
    ///
    /// The device returns:
    ///
    /// - a count line (`[BeaconCount:2]` in hex)
    /// - followed by `BeaconCount` beacon records
    /// - and an empty line terminator
    ///
    /// This parser supports both documented record layouts:
    ///
    /// - Standard format (17 fields)
    /// - FW2.6 format (26 fields, e.g. after `CONFSET;B3;1`)
    ///
    /// If the record count does not match, or a record has an unknown shape,
    /// this returns an error.
    pub fn beacon_get(&mut self) -> Result<Vec<BeaconRecord>, Error> {
        let response = self.command("BEACONGET")?;
        self.ensure_code(&response, 0x00)?;
        let count_line =
            response.data_lines.first().ok_or_else(|| Error::Protocol("BEACONGET missing count line".to_string()))?;

        let expected_count = parse_hex_u8(count_line)? as usize;
        let mut out = Vec::new();
        for line in response.data_lines.iter().skip(1) {
            out.push(BeaconRecord::from_line(line)?);
        }
        if out.len() != expected_count {
            return Err(Error::Protocol(format!(
                "BEACONGET count mismatch: header={expected_count}, records={}",
                out.len()
            )));
        }
        Ok(out)
    }

    /// From the docs:
    /// Replies always start with an echo of the command followed by a semicolon and a return code indicating success or error.
    /// The next lines contain the response of the call. Every response line is terminated with a single \n newline character.
    /// The reply is finally terminated with another \n  newline character. It is best practice to wait for a double \n\n new
    /// line as indication for the end of the call. \n\n will never be part of the response data.
    fn read_command_response(&mut self, expected_command: &str) -> Result<CommandResponse, Error> {
        let header = loop {
            let line = self.read_line()?;
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            break line;
        };

        let mut parts = header.split(';');
        let command = parts.next().ok_or_else(|| Error::Protocol("missing command in reply".to_string()))?.to_string();
        if command != expected_command {
            return Err(Error::Protocol(format!(
                "unexpected command echo: expected {expected_command}, got {command}"
            )));
        }

        let return_code = parts
            .next()
            .ok_or_else(|| Error::Protocol("missing return code in reply".to_string()))
            .and_then(parse_hex_u8)?;

        // Wait for second newline
        let mut data_lines = Vec::new();
        loop {
            let line = self.read_line()?;
            if line.is_empty() {
                break;
            }
            data_lines.push(line);
        }

        Ok(CommandResponse { command, return_code, data_lines })
    }

    fn read_line(&mut self) -> Result<String, Error> {
        loop {
            if let Some(line) = self.consume_line_from_scratch() {
                return Ok(line);
            }

            let mut buf = [0_u8; 256];
            match self.port.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => self.scratch.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => return Err(Error::Io(e)),
            }
        }
    }

    fn try_read_line(&mut self) -> Result<Option<String>, Error> {
        if let Some(line) = self.consume_line_from_scratch() {
            return Ok(Some(line));
        }
        let mut buf = [0_u8; 256];
        match self.port.read(&mut buf) {
            Ok(0) => Ok(None),
            Ok(n) => {
                self.scratch.extend_from_slice(&buf[..n]);
                Ok(self.consume_line_from_scratch())
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(None),
            Err(e) => Err(Error::Io(e)),
        }
    }

    fn consume_line_from_scratch(&mut self) -> Option<String> {
        let pos = self.scratch.iter().position(|&b| b == b'\n')?;
        let mut bytes: Vec<u8> = self.scratch.drain(..=pos).collect();
        if bytes.last().copied() == Some(b'\n') {
            bytes.pop();
        }
        if bytes.last().copied() == Some(b'\r') {
            bytes.pop();
        }
        Some(String::from_utf8_lossy(&bytes).to_string())
    }

    fn ensure_code(&self, response: &CommandResponse, expected_code: u8) -> Result<(), Error> {
        if response.return_code == expected_code {
            Ok(())
        } else {
            Err(Error::CommandFailed {
                command: response.command.clone(),
                return_code: response.return_code,
                data_lines: response.data_lines.clone(),
            })
        }
    }
}
