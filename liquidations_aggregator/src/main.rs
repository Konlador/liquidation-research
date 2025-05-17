use {
    futures::stream::{self, StreamExt},
    std::collections::HashSet,
    std::env,
    std::error::Error as StdError,
    std::process::Stdio,
    std::sync::Arc,
    tokio::process::Command,
    tokio::sync::{Mutex, Semaphore},
};

mod big_num;
mod db_client;
mod log_parsing;
mod memory;

pub use big_num::*;
pub use db_client::*;
pub use log_parsing::*;
pub use memory::*;

async fn run_forge_test(liquidation_data: &LiquidationData) -> Result<String, Box<dyn StdError>> {
    println!("Runing test for tx {}", liquidation_data.transaction_hash);

    // Get the current directory and move one level up
    let current_dir = env::current_dir()?;
    let parent_dir = current_dir
        .parent()
        .ok_or("Failed to get parent directory")?;

    // Execute the command in the parent directory and wait for it to complete
    let mut cmd = Command::new("forge")
        .arg("test")
        .arg("--no-rpc-rate-limit")
        .arg("--match-test")
        .arg("testLiquidations")
        .arg("-vv")
        .env("TX_HASH", &liquidation_data.transaction_hash)
        .env("REPAY_V_TOKEN", &liquidation_data.v_token)
        .env("BORROWER", &liquidation_data.borrower)
        .env("REPAY_AMOUNT", liquidation_data.repay_amount.to_string())
        .env("COLLATERAL_V_TOKEN", &liquidation_data.v_token_collateral)
        .env("EXPECTED_SEIZE", liquidation_data.seize_tokens.to_string())
        .env("GAS_PRICE", liquidation_data.gas_price.to_string())
        .current_dir(parent_dir) // Set the working directory to the parent
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Monitor memory usage
    let pid = cmd.id().expect("Failed to get child PID");
    let mut sys = sysinfo::System::new();
    // let mem_check = tokio::spawn(async move {

    // });

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // if is_memory_under_pressure().await.unwrap_or(false) {
        //     eprintln!("System under memory pressure (pageouts detected). Killing child.");
        //     let _ = cmd.kill().await;
        //     break;
        // }

        let child_status = cmd.try_wait();
        if let Ok(None) = child_status {
            // println!("Child did not finish yet {}", pid);
            sys.refresh_process(sysinfo::Pid::from_u32(pid));
            if let Some(proc) = sys.process(sysinfo::Pid::from_u32(pid)) {
                let memory_bytes = proc.memory();
                // println!("Child memory usage {}", memory_bytes);
                if memory_bytes > 10 * 1024 * 1024 * 1024 {
                    eprintln!(
                        "Killing child process {} due to memory > 10GB ({})",
                        pid, memory_bytes
                    );
                    let _ = cmd.kill().await;
                    break;
                }
            } else {
                break; // process exited
            }

            continue;
        } else {
            break;
        }
    }

    //     let output = {
    //         use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    // use std::ffi::OsStr;
    // use std::future::Future;
    // use std::io;
    // use std::path::Path;
    // use std::pin::Pin;
    // use std::process::{Command as StdCommand, ExitStatus, Output, Stdio};
    // use std::task::{ready, Context, Poll};

    //         async fn read_to_end<A: AsyncRead + Unpin>(io: &mut Option<A>) -> io::Result<Vec<u8>> {
    //             let mut vec = Vec::new();
    //             if let Some(io) = io.as_mut() {
    //                 tokio::io::util::read_to_end(io, &mut vec).await?;
    //             }
    //             Ok(vec)
    //         }

    //         let mut stdout_pipe = cmd.stdout.take();
    //         let mut stderr_pipe = cmd.stderr.take();

    //         let stdout_fut = read_to_end(&mut stdout_pipe);
    //         let stderr_fut = read_to_end(&mut stderr_pipe);

    //         let (status, stdout, stderr) = try_join3(self.wait(), stdout_fut, stderr_fut).await?;

    //         // Drop happens after `try_join` due to <https://github.com/tokio-rs/tokio/issues/4309>
    //         drop(stdout_pipe);
    //         drop(stderr_pipe);

    //         Ok(Output {
    //             status,
    //             stdout,
    //             stderr,
    //         })
    //     }

    let output = cmd.wait_with_output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "Command failed with status: {:?}\nStdout: {}\nStderr: {}",
            output.status.code(),
            "<no stdout>",
            stderr
        )
        .into())
    }
}

#[tokio::main]
async fn main() {
    let pool = Arc::new(create_pool());

    let liquidation_data = match fetch_liquidation_data(pool.clone()).await {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error fetching liquidation data: {}", e);
            return;
        }
    };

    println!(
        "Starting to analyze {} liquidations",
        liquidation_data.len(),
    );

    let parallel_workers: usize = 16;
    let semaphore = Arc::new(Semaphore::new(parallel_workers));
    let active_blocks = Arc::new(Mutex::new(HashSet::new()));

    stream::iter(liquidation_data)
        .for_each_concurrent(Some(parallel_workers), |data| {
            let semaphore = Arc::clone(&semaphore);
            let active_blocks = Arc::clone(&active_blocks);
            let pool_clone = pool.clone();
            async move {
                // Wait if memory is above threshold
                wait_if_memory_high(30).await;

                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        eprintln!("Error acquiring semaphore: {}", e);
                        return;
                    }
                };

                let mut active_blocks_guard = active_blocks.lock().await;
                if active_blocks_guard.insert(data.block_number) {
                    drop(active_blocks_guard); // Release the lock as soon as possible
                } else {
                    // If the block_number is already in the set, skip processing
                    println!(
                        "Skipping duplicate test for block number {}. Tx {}",
                        data.block_number, data.transaction_hash,
                    );
                    return;
                }

                let logs = match run_forge_test(&data).await {
                    Ok(logs) => logs,
                    Err(e) => {
                        eprintln!(
                            "Error running forge test for {}: {}",
                            data.transaction_hash, e
                        );
                        return;
                    }
                };

                // Parse the logs and insert data into the database
                let parsed_data = parse_logs(&logs);
                if let Err(e) =
                    insert_with_retries(pool_clone, &data.transaction_hash, &parsed_data).await
                {
                    eprintln!(
                        "Error inserting data into database for {}: {}",
                        data.transaction_hash, e
                    );
                    return;
                }

                let mut active_blocks_guard = active_blocks.lock().await;
                active_blocks_guard.remove(&data.block_number);
                drop(active_blocks_guard);

                println!("Data inserted for {}", data.transaction_hash);
            }
        })
        .await;
}
