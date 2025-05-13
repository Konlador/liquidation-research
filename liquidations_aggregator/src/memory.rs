use std::time::Duration;
use sysinfo::System;
use tokio::time::sleep;

pub async fn wait_if_memory_high(threshold_gb: u64) {
    let mut sys = System::new();
    loop {
        sys.refresh_process(sysinfo::get_current_pid().unwrap());
        let current_pid = sysinfo::get_current_pid().unwrap();
        let process = sys.process(current_pid).unwrap();

        let used_bytes = process.memory() * 1024; // in bytes
        let threshold_bytes = threshold_gb * 1024 * 1024 * 1024;

        println!("Memory usage({} GB)", used_bytes / 1024 / 1024 / 1024);

        // let child_count = sys
        //     .processes()
        //     .values()
        //     .filter(|process| process.parent() == Some(current_pid))
        //     .count();

        // println!(
        //     "Child processes running: {}, current_pid: {}",
        //     child_count, current_pid
        // );

        if used_bytes < threshold_bytes {
            break;
        }

        println!(
            "Memory usage too high ({} GB), pausing...",
            used_bytes / 1024 / 1024 / 1024
        );
        sleep(Duration::from_millis(500)).await;
    }
}

use std::str;
use tokio::process::Command;

pub async fn is_memory_under_pressure() -> Result<bool, Box<dyn std::error::Error>> {
    let output = Command::new("vm_stat").output().await?;

    let stdout = str::from_utf8(&output.stdout)?;
    println!("{}", stdout);
    for line in stdout.lines() {
        if line.contains("Pageouts") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 {
                let val = parts[1]
                    .trim()
                    .trim_end_matches('.')
                    .replace(".", "")
                    .replace(",", "");
                if let Ok(pageouts) = val.parse::<u64>() {
                    if pageouts > 0 {
                        return Ok(true); // Swapping has occurred => memory pressure
                    }
                }
            }
        }
    }

    Ok(false)
}
