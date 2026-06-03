use std::time::Duration;
use usb_timing_box_rs::{UsbTimingBox, commands::PassingGetResult};

const POLL_INTERVAL: Duration = Duration::from_millis(150);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut box_client = UsbTimingBox::builder("COM3").connect()?;

    box_client.wait_for_autoboot(Duration::from_secs(4))?;
    box_client.switch_to_ascii_protocol()?;

    let mut box_client = box_client.enable_fw26_data_format()?;

    // let site_survey = box_client.site_survey()?;
    // println!("Site survey: {:?}", site_survey);

    println!("Decoder ID: 0x{:04x}", box_client.info_get_decoder_id()?);
    println!("Firmware version: {:.1}", box_client.info_get_firmware_version()?);
    println!("Hardware version: {:.1}", box_client.info_get_hardware_version()?);
    println!("Box type: {:?}", box_client.info_get_box_type()?);
    println!("Battery voltage: {:.1} V", box_client.info_get_battery_voltage()?);
    println!("Battery state: {:?}", box_client.info_get_battery_state()?);
    println!("Battery level: {} %", box_client.info_get_battery_level()?);
    println!("Internal temperature: {} °C", box_client.info_get_internal_temperature()?);
    println!("Supply voltage: {:.1} V", box_client.info_get_supply_voltage()?);
    println!("Loop status: {:?}", box_client.info_get_loop_status()?);
    println!("Built revision: {}", box_client.info_get_built_revision()?);
    println!("Measured loop power: {} %", box_client.info_get_measured_loop_power()?);

    // Doesn't seem to work on usb timing box although documentation states it does.
    // println!("Noise status: {}", box_client.info_get_noise_status()?);

    let passing_info = box_client.passing_info_get()?;
    println!("Passing info: {:?}", passing_info);

    let epoch = box_client.epoch_ref_sync_to_next_second()?;

    let startup_timestamp = box_client.timestamp_get()?;

    let mut next_index = passing_info.start_id.wrapping_add(passing_info.count as u32);

    let beacons = box_client.beacon_get()?;
    println!("Beacons: {beacons:?}");

    println!(
        "Startup state: count={}, start_id={}, last_id={}, startup_timestamp=0x{startup_timestamp:08x}, next_index={next_index}",
        passing_info.count, passing_info.start_id, passing_info.last_id
    );

    loop {
        match box_client.passing_get(next_index)? {
            PassingGetResult::Ok(batch) => {
                if batch.passings.is_empty() {
                    std::thread::sleep(POLL_INTERVAL);
                    continue;
                }
                for passing in &batch.passings {
                    let timestamp = passing.datetime_utc(epoch)?.with_timezone(&chrono::Local);
                    println!("{timestamp}: {passing:?}");
                }
                next_index = batch.next_start_index();
            }
            PassingGetResult::StartIndexNotFound { .. } => {
                // This also gets returned when the start index does not yet exist.
                std::thread::sleep(POLL_INTERVAL);
            }
            PassingGetResult::WrongMode => {
                eprintln!("Device is in a mode that does not allow PASSINGGET.");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}
