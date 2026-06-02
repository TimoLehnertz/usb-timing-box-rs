use std::time::Duration;
use usb_timing_box_rs::{UsbTimingBox, commands::PassingGetResult};

fn live_tail_index(start_id: u32, count: u16) -> u32 {
    // One past the newest buffered passing, modulo u32.
    start_id.wrapping_add(count as u32)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut box_client = UsbTimingBox::builder("COM3").connect()?;

    box_client.wait_for_autoboot(Duration::from_secs(4))?;
    box_client.switch_to_ascii_protocol()?;

    let site_survey = box_client.site_survey()?;
    println!("Site survey: {:?}", site_survey);

    if let Ok(epoch) = box_client.epoch_ref_get() {
        println!("Current epoch ref: unix={} timestamp=0x{:08x}", epoch.unix_time_seconds, epoch.timestamp_ticks);
    }

    let passing_info = box_client.passing_info_get()?;
    println!("Passing info: {:?}", passing_info);

    let startup_timestamp = box_client.timestamp_get()? as u64;

    let mut next_index = live_tail_index(passing_info.start_id, passing_info.count);

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
                    std::thread::sleep(Duration::from_millis(150));
                    continue;
                }

                let mut printed = 0usize;
                for passing in &batch.passings {
                    // Keep only passings newer than program start.
                    if let Some(ticks) = passing.timestamp_ticks() {
                        if ticks <= startup_timestamp {
                            continue;
                        }
                    }
                    println!("Passing: {passing}");
                    printed += 1;
                }
                println!(
                    "batch_size={}, printed={printed} next_start_index={}",
                    batch.passings.len(),
                    batch.next_start_index()
                );
                next_index = batch.next_start_index();
            }
            PassingGetResult::StartIndexTooLow { .. } => {
                // println!(
                //     "StartIndexTooLow: min_start_index={min_start_index} echoed_start={echoed_start} requested_start={requested_start}"
                // );
                // Do not jump to min_start_index (history replay); re-anchor to live tail.
                let info = box_client.passing_info_get()?;
                next_index = live_tail_index(info.start_id, info.count);
            }
            PassingGetResult::WrongMode => {
                eprintln!("Device is in a mode that does not allow PASSINGGET.");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}
