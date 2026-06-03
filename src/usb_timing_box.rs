use std::{
    marker::PhantomData,
    time::{Duration, Instant},
};

use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

use crate::{
    ConfigParameter, DEFAULT_BAUD_RATE,
    commands::{
        BatteryState, BeaconRecordFw25, BeaconRecordFw26, BoxType, CommandResponse, EpochReferenceFw25,
        EpochReferenceFw26, LoopStatus, OperationMode, PassingGetResult, PassingInfo, passing_get_from_response_fw25,
        passing_get_from_response_fw26,
    },
    error::Error,
    firmware::{Fw25, Fw26},
    info_parameter::{
        InfoParameter, parse_battery_level_percent, parse_battery_state, parse_box_type, parse_decoder_id,
        parse_internal_temperature_fw25, parse_internal_temperature_fw26, parse_loop_status,
        parse_measured_loop_power_percent, parse_noise_status, parse_version_tenths, parse_voltage_tenths,
    },
    utils::{hex2, hex8, parse_hex_u8, parse_hex_u32, parse_hex_u64, unix_time_now},
};

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

    pub fn connect(self) -> Result<UsbTimingBox<Fw25>, Error> {
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

        Ok(UsbTimingBox { port, scratch: Vec::new(), _firmware: PhantomData })
    }
}

/// USB Timing Box client parameterized by firmware data format ([`Fw25`] default, [`Fw26`] after
/// [`UsbTimingBox::enable_fw26_data_format`]).
#[derive(Debug)]
pub struct UsbTimingBox<F = Fw25> {
    port: Box<dyn SerialPort>,
    scratch: Vec<u8>,
    _firmware: PhantomData<F>,
}

impl UsbTimingBox<Fw25> {
    pub fn builder(port_name: impl Into<String>) -> UsbTimingBoxBuilder {
        UsbTimingBoxBuilder::new(port_name)
    }

    pub fn connect(port_name: impl Into<String>) -> Result<Self, Error> {
        UsbTimingBoxBuilder::new(port_name).connect()
    }
}

impl<F> UsbTimingBox<F> {
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

    fn info_get_raw(&mut self, parameter: InfoParameter) -> Result<String, Error> {
        let parameter_id = parameter.id();
        let response = self.command_with_args("INFOGET", &[hex2(parameter_id)])?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("INFOGET missing data line".to_string()))?;
        let mut parts = line.split(';');
        let id =
            parts.next().ok_or_else(|| Error::Protocol("INFOGET missing id".to_string())).and_then(parse_hex_u8)?;

        if id != parameter_id {
            return Err(Error::Protocol(format!("INFOGET id mismatch: expected {parameter_id:02x}, got {id:02x}")));
        }

        let value = parts.next().ok_or_else(|| Error::Protocol("INFOGET missing value".to_string()))?;
        Ok(value.to_string())
    }

    /// Decoder ID (4 hex digits, e.g. `1387` → `A-4999`). See [`InfoParameter::DecoderId`].
    pub fn info_get_decoder_id(&mut self) -> Result<u16, Error> {
        parse_decoder_id(&self.info_get_raw(InfoParameter::DecoderId)?)
    }

    /// Firmware version (e.g. `18` → 2.4). See [`InfoParameter::FirmwareVersion`].
    pub fn info_get_firmware_version(&mut self) -> Result<f32, Error> {
        parse_version_tenths(&self.info_get_raw(InfoParameter::FirmwareVersion)?)
    }

    /// Hardware version. See [`InfoParameter::HardwareVersion`].
    pub fn info_get_hardware_version(&mut self) -> Result<f32, Error> {
        parse_version_tenths(&self.info_get_raw(InfoParameter::HardwareVersion)?)
    }

    /// Device type. See [`InfoParameter::BoxType`].
    pub fn info_get_box_type(&mut self) -> Result<BoxType, Error> {
        parse_box_type(&self.info_get_raw(InfoParameter::BoxType)?)
    }

    /// Battery voltage in volts. See [`InfoParameter::BatteryVoltage`].
    pub fn info_get_battery_voltage(&mut self) -> Result<f32, Error> {
        parse_voltage_tenths(&self.info_get_raw(InfoParameter::BatteryVoltage)?)
    }

    /// Battery state. See [`InfoParameter::BatteryState`].
    pub fn info_get_battery_state(&mut self) -> Result<BatteryState, Error> {
        parse_battery_state(&self.info_get_raw(InfoParameter::BatteryState)?)
    }

    /// Battery level 0–100 %. See [`InfoParameter::BatteryLevel`].
    pub fn info_get_battery_level(&mut self) -> Result<u8, Error> {
        parse_battery_level_percent(&self.info_get_raw(InfoParameter::BatteryLevel)?)
    }

    /// Supply voltage in volts. See [`InfoParameter::SupplyVoltage`].
    pub fn info_get_supply_voltage(&mut self) -> Result<f32, Error> {
        parse_voltage_tenths(&self.info_get_raw(InfoParameter::SupplyVoltage)?)
    }

    /// Loop status. See [`InfoParameter::LoopStatus`].
    pub fn info_get_loop_status(&mut self) -> Result<LoopStatus, Error> {
        parse_loop_status(&self.info_get_raw(InfoParameter::LoopStatus)?)
    }

    /// Build revision string (internal). See [`InfoParameter::BuiltRevision`].
    pub fn info_get_built_revision(&mut self) -> Result<String, Error> {
        self.info_get_raw(InfoParameter::BuiltRevision)
    }

    /// Measured loop power 0–100 %. See [`InfoParameter::MeasuredLoopPower`].
    pub fn info_get_measured_loop_power(&mut self) -> Result<u8, Error> {
        parse_measured_loop_power_percent(&self.info_get_raw(InfoParameter::MeasuredLoopPower)?)
    }

    /// Channel noise 0–10 (FW 2.6+). See [`InfoParameter::NoiseStatus`].
    pub fn info_get_noise_status(&mut self) -> Result<u8, Error> {
        parse_noise_status(&self.info_get_raw(InfoParameter::NoiseStatus)?)
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

        PassingInfo::from_line(line)
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
                Err(e) => return Err(e.into()),
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
            Err(e) => Err(e.into()),
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

impl UsbTimingBox<Fw25> {
    /// Enables FW 2.6 data reporting on the device and returns a client typed for that format.
    ///
    /// See [`ConfigParameter::EnableFw26DataFormat`] (signed temperature, extended beacons,
    /// 2048 Hz timestamps with day adjustment).
    pub fn enable_fw26_data_format(mut self) -> Result<UsbTimingBox<Fw26>, Error> {
        self.conf_set(ConfigParameter::EnableFw26DataFormat, 1)?;
        Ok(UsbTimingBox { port: self.port, scratch: self.scratch, _firmware: PhantomData })
    }

    /// Internal temperature 0–100 °C (FW 2.5). See [`InfoParameter::InternalTemperature`].
    pub fn info_get_internal_temperature(&mut self) -> Result<u8, Error> {
        parse_internal_temperature_fw25(&self.info_get_raw(InfoParameter::InternalTemperature)?)
    }

    pub fn epoch_ref_get(&mut self) -> Result<EpochReferenceFw25, Error> {
        let response = self.command("EPOCHREFGET")?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFGET missing data line".to_string()))?;
        EpochReferenceFw25::parse(line)
    }

    pub fn epoch_ref_set(&mut self, epoch_seconds: u32) -> Result<EpochReferenceFw25, Error> {
        let response = self.command_with_args("EPOCHREFSET", &[hex8(epoch_seconds)])?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFSET missing data line".to_string()))?;
        EpochReferenceFw25::parse(line)
    }

    pub fn epoch_ref_set_with_dtr_pulse(
        &mut self,
        epoch_seconds: u32,
        pulse_length: Duration,
    ) -> Result<EpochReferenceFw25, Error> {
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
        EpochReferenceFw25::parse(body)
    }

    /// Sets the computer time reference using the recommended `EPOCHREFSET` + DTR workflow.
    ///
    /// The USB Timing Box has no real-time clock. Passing timestamps are only meaningful after
    /// you store a reference pair `(unix_time_seconds, timestamp_ticks)` in the device. That pair
    /// is created by [`EPOCHREFSET`](https://www.raceresult.com/en/support/kb?id=422-Command-EPOCHREFSET)
    /// together with a DTR pulse and can later be read back with
    /// [`EPOCHREFGET`](https://www.raceresult.com/en/support/kb?id=421-Command-EPOCHREFGET).
    ///
    /// # What this function does
    ///
    /// Implements the documented best-practice sequence from
    /// [Command EPOCHREFSET](https://www.raceresult.com/en/support/kb?id=422-Command-EPOCHREFSET):
    ///
    /// 1. Sends `EPOCHREFSET;<next_unix_second>\n` where `next_unix_second` is the current host
    ///    UNIX time plus one second (aligned to the *upcoming* full second).
    /// 2. Busy-waits until the host clock reaches that second.
    /// 3. Pulses the serial **DTR line HIGH for 200 ms**, then returns it LOW. The box captures
    ///    its internal time base on the **rising edge** of DTR; this must happen within **2 s**
    ///    of the `EPOCHREFSET` command or the device replies with error `10` (DTR timeout).
    /// 4. Reads the command reply and returns the stored pair as [`EpochReferenceFw25`].
    ///
    /// The returned values match the protocol data line
    /// `[ComputerTime:8];[TimeStamp:8]` (`unix_time_seconds`; `timestamp_ticks` at 256 ticks/s).
    ///
    /// # Converting passings afterward
    ///
    /// Use [`EpochReferenceFw25::passing_time_seconds`] (or the FW 2.6 equivalent) with each
    /// passing's box timestamp:
    ///
    /// `pass_time = ref_computer_time + (pass_timestamp - ref_timestamp) / ticks_per_second`
    ///
    /// # When to call
    ///
    /// Call once after connect if [`Self::epoch_ref_get`] reports zeros (no reference stored yet),
    /// e.g. after a fresh boot. If a reference is already present (e.g. host crashed mid-event),
    /// prefer [`Self::epoch_ref_get`] and only re-sync when you intentionally want a new anchor.
    ///
    /// # DTR requirement
    ///
    /// Accurate sync requires controlling DTR (see crate docs on DTR-line reset). If DTR cannot
    /// be driven, disable box DTR handling via [`ConfigParameter::UseDtr`] and use
    /// [`Self::epoch_ref_set`] without sub-second alignment instead (lower accuracy).
    ///
    /// # See also
    ///
    /// - [`Self::epoch_ref_get`] — read the pair stored by the last successful sync
    /// - [`Self::epoch_ref_set`] — set epoch without waiting for a boundary (no DTR timing)
    /// - [`Self::epoch_ref_set_with_dtr_pulse`] — set a specific epoch with a custom DTR pulse length
    pub fn epoch_ref_sync_to_next_second(&mut self) -> Result<EpochReferenceFw25, Error> {
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
        EpochReferenceFw25::parse(body)
    }

    /// Current box timestamp (32-bit, 256 ticks per second).
    ///
    /// ## Note
    /// Do not use for timing calculation purposes! Your operating system serial
    /// port data buffer is introducing unpredictable delays.
    pub fn timestamp_get(&mut self) -> Result<u32, Error> {
        let response = self.command("TIMESTAMPGET")?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("TIMESTAMPGET missing data line".to_string()))?;
        parse_hex_u32(line)
    }

    pub fn passing_get(&mut self, start_index: u32) -> Result<PassingGetResult<Fw25>, Error> {
        let response = self.command_with_args("PASSINGGET", &[hex8(start_index)])?;
        passing_get_from_response_fw25(start_index, response)
    }

    /// Queries status beacons (`BEACONGET`, 17-field FW 2.5 layout).
    ///
    /// This is unclear in the official docs:
    ///
    /// Why does this return a list of beacons? Probably so that one device can send
    /// beacons for multiple other devices that are connected to it.
    pub fn beacon_get(&mut self) -> Result<Vec<BeaconRecordFw25>, Error> {
        let response = self.command("BEACONGET")?;
        self.ensure_code(&response, 0x00)?;
        let count_line =
            response.data_lines.first().ok_or_else(|| Error::Protocol("BEACONGET missing count line".to_string()))?;
        let expected_count = parse_hex_u8(count_line)? as usize;
        let mut out = Vec::new();
        for line in response.data_lines.iter().skip(1) {
            out.push(BeaconRecordFw25::from_line(line)?);
        }
        if out.len() != expected_count {
            return Err(Error::Protocol(format!(
                "BEACONGET count mismatch: header={expected_count}, records={}",
                out.len()
            )));
        }
        Ok(out)
    }
}

impl UsbTimingBox<Fw26> {
    /// Internal temperature in °C (FW 2.6 signed hex). See [`InfoParameter::InternalTemperature`].
    pub fn info_get_internal_temperature(&mut self) -> Result<i8, Error> {
        parse_internal_temperature_fw26(&self.info_get_raw(InfoParameter::InternalTemperature)?)
    }

    pub fn epoch_ref_get(&mut self) -> Result<EpochReferenceFw26, Error> {
        let response = self.command("EPOCHREFGET")?;
        self.ensure_code(&response, 0x00)?;

        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFGET missing data line".to_string()))?;

        EpochReferenceFw26::parse(line)
    }

    pub fn epoch_ref_set(&mut self, epoch_seconds: u32) -> Result<EpochReferenceFw26, Error> {
        let response = self.command_with_args("EPOCHREFSET", &[hex8(epoch_seconds)])?;
        self.ensure_code(&response, 0x00)?;

        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("EPOCHREFSET missing data line".to_string()))?;

        EpochReferenceFw26::parse(line)
    }

    pub fn epoch_ref_set_with_dtr_pulse(
        &mut self,
        epoch_seconds: u32,
        pulse_length: Duration,
    ) -> Result<EpochReferenceFw26, Error> {
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
        EpochReferenceFw26::parse(body)
    }

    pub fn epoch_ref_sync_to_next_second(&mut self) -> Result<EpochReferenceFw26, Error> {
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
        EpochReferenceFw26::parse(body)
    }

    /// Current box timestamp (40-bit, 2048 ticks per second, 10 hex digits on the wire).
    pub fn timestamp_get(&mut self) -> Result<u64, Error> {
        let response = self.command("TIMESTAMPGET")?;
        self.ensure_code(&response, 0x00)?;
        let line =
            response.single_data_line().ok_or_else(|| Error::Protocol("TIMESTAMPGET missing data line".to_string()))?;
        parse_hex_u64(line)
    }

    pub fn passing_get(&mut self, start_index: u32) -> Result<PassingGetResult<Fw26>, Error> {
        let response = self.command_with_args("PASSINGGET", &[hex8(start_index)])?;
        passing_get_from_response_fw26(start_index, response)
    }

    /// Queries status beacons (`BEACONGET`, 26-field FW 2.6 layout).
    ///
    /// This is unclear in the official docs:
    ///
    /// Why does this return a list of beacons? Probably so that one device can send
    /// beacons for multiple other devices that are connected to it.
    pub fn beacon_get(&mut self) -> Result<Vec<BeaconRecordFw26>, Error> {
        let response = self.command("BEACONGET")?;
        self.ensure_code(&response, 0x00)?;
        let count_line =
            response.data_lines.first().ok_or_else(|| Error::Protocol("BEACONGET missing count line".to_string()))?;

        let expected_count = parse_hex_u8(count_line)? as usize;
        let mut out = Vec::new();
        for line in response.data_lines.iter().skip(1) {
            out.push(BeaconRecordFw26::from_line(line)?);
        }
        if out.len() != expected_count {
            return Err(Error::Protocol(format!(
                "BEACONGET count mismatch: header={expected_count}, records={}",
                out.len()
            )));
        }
        Ok(out)
    }
}
