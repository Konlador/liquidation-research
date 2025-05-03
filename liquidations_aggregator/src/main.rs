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

pub use big_num::*;
pub use db_client::*;
pub use log_parsing::*;

async fn run_forge_test(liquidation_data: &LiquidationData) -> Result<String, Box<dyn StdError>> {
    println!("Runing test for tx {}", liquidation_data.transaction_hash);

    // Get the current directory and move one level up
    let current_dir = env::current_dir()?;
    let parent_dir = current_dir
        .parent()
        .ok_or("Failed to get parent directory")?;

    // Execute the command in the parent directory and wait for it to complete
    let cmd = Command::new("forge")
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

    let parallel_workers: usize = 12;
    let semaphore = Arc::new(Semaphore::new(parallel_workers));
    let active_blocks = Arc::new(Mutex::new(HashSet::new()));

    stream::iter(liquidation_data)
        .for_each_concurrent(Some(parallel_workers), |data| {
            let semaphore = Arc::clone(&semaphore);
            let active_blocks = Arc::clone(&active_blocks);
            let pool_clone = pool.clone();
            async move {
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
