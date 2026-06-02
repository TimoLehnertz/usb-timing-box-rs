#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConfigParameter {
    /// Push pre-warn messages prior to real passing (non persistent).
    ///
    /// Allowed values:
    /// - `00` = disabled (default)
    /// - `01` = enabled
    PushPrewarn = 0x01,

    /// Blink LED on reception of a repeated passing (non persistent).
    ///
    /// Allowed values:
    /// - `00` = disabled
    /// - `01` = enabled (default)
    BlinkOnRepeatedPassing = 0x02,

    /// Use the headphone jack as impulse input to generate a fake passing.
    ///
    /// Allowed values:
    /// - `00` = impulse-in
    /// - `01` = beep-out (default)
    ImpulseInputOrBeepOutput = 0x03,

    /// Switch off the box on power loss. Can be used if timing is finished and you want the USB Timing Box to shut down with the computer
    /// (non persistent).
    ///
    /// Allowed values:
    /// - `00` = disabled (default)
    /// - `01` = enabled
    AutoShutdownOnPowerLoss = 0x04,

    /// See [OperationMode]
    ///
    /// Allowed values:
    /// - `05` = usb-kiosk
    /// - `06` = usb-timing (default)
    /// - `07` = usb-store&copy
    /// - `08` = usb-repeat-impulse
    OperationMode = 0x05,

    /// CH# used for wireless communication. Please note that the channels on the display are shown with an offset of +1, from 1-8.
    ///
    /// Allowed values: `00..=07` (displayed as `1..=8`)
    ChannelId = 0x06,

    /// Loop# of the wire loop. Please note that the IDs on the display are shown with an offset of +1, from 1-8.
    ///
    /// Allowed values: `00..=07` (displayed as `1..=8`)
    LoopId = 0x07,

    /// Sets the power of the loop, from 0:off to 100:full (hex 64) power.
    ///
    /// Allowed values: `00..=64`
    LoopPower = 0x08,

    /// Leave set to 15 (hex 0F), don't change!
    ///
    /// Allowed values: `02..=FF` (default `0F`)
    BlinkDeadTime = 0x09,

    /// If the timing computer itself is running on battery, it might not be good idea to charge the USB Timing Box.
    ///
    /// Allowed values:
    /// - `00` = disabled
    /// - `01` = enabled (default)
    UsbCharging = 0x0a,

    /// The DTR line is used for improved timing accuracy, see Timebase Control section.
    ///
    /// Allowed values:
    /// - `00` = disabled
    /// - `01` = enabled (default)
    UseDtr = 0x0b,

    /// Sets the power of the loop, from 0:off to 100:full (hex 64) power.
    ///
    /// Allowed values: `01..=64` (default `0F`)
    TrayScanPower = 0xa0,

    /// Time span for which a single row or column is powered.
    ///
    /// Allowed values: `01..=FF` in ticks (1/256th) (default `50`)
    TrayScanInterval = 0xa1,

    /// Time to ramp up loop power, before first scan
    ///
    /// Allowed values: `01..=FF` in ticks (1/256th) (default `E0`)
    TrayScanRampUpDelay = 0xa2,

    /// Rows are scanned with scan interval first, then there is a delay, then columns are scanned with scan interval.
    ///
    /// Allowed values: `01..=FF` in ticks (1/256th) (default `E0`)
    TrayScanRowColumnDelay = 0xa3,

    /// Cycle thru rows 1 + N times.
    /// Cycle thru column 1 + N times.
    ///
    /// Allowed values: `01..=0A` (default `00`)
    TrayScanRepeatCycles = 0xa4,
    /// Enable CheckSum for all ASCII Protocol communication (see here)
    /// Only applies to FW 2.5.22868 and up.
    ///
    /// Allowed values: `0..=1` (default `0`)
    EnableChecksum = 0xb1,

    /// Push Passings enable. Only applies to FW 2.6 and up.
    /// Values of 1 and 255 lead to endless pushing of passings
    /// Values > 1 and <= 254 tell the box to push exactly this number of passings. You need to refresh this value again if you want to receive the next passings.
    ///
    /// Allowed values (default `0`):
    /// - `00` = disabled
    /// - `01` or `FF` (255) = push passings endlessly
    /// - `02..=FE` (2..254) = push exactly this many passings
    PushPassings = 0xb2,

    /// Enable FW Version 2.6 data reporting format (Signed Temperature, New Beacons Info, Timestamps with 2048th and dayadj)
    /// 0 - 1 (default 0)
    EnableFw26DataFormat = 0xb3,

    /// Enable Status Push Message every second:
    /// `#S;[TickCount:8];[MainLoopCycles:4];[LoopStatus:2];[MeasuredLoopPower:2];[ChannelNoise:2];[BattPercent:2]\n\n`
    /// #S;015405f2;11f7;00;2c;01;3b\n\n
    ///
    /// Allowed values: `0..=1` (default `0`)
    EnableStatusPush = 0xb4,
}

impl ConfigParameter {
    pub fn id(self) -> u8 {
        self as u8
    }
}
