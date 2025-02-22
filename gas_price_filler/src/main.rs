pub use db_client::*;
use dotenv::dotenv;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

mod db_client;

#[derive(Debug)]
struct TransactionInfo {
    gas_price: u64,
    data_prefix: String,
}

async fn get_transaction_info(
    tx_hash: &str,
    rpc_url: &str,
) -> Result<TransactionInfo, Box<dyn Error>> {
    let client = Client::new();

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionByHash",
        "params": [tx_hash],
        "id": 1
    });

    let response = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .await?
        .json::<Value>()
        .await?;

    let result = response["result"]
        .as_object()
        .ok_or("Failed to retrieve transaction")?;

    // Extract gas price and convert hex to decimal
    let gas_price_hex = result["gasPrice"]
        .as_str()
        .ok_or("Failed to retrieve gas price")?;
    let gas_price = u64::from_str_radix(gas_price_hex.trim_start_matches("0x"), 16)?;

    // Extract the first 4 bytes of tx data
    let data = result["input"]
        .as_str()
        .ok_or("Failed to retrieve tx data")?;
    let data_prefix = data.get(0..10).unwrap_or("0x").to_string();

    Ok(TransactionInfo {
        gas_price,
        data_prefix,
    })
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let rpc_url = &env::var("RPC_URL").expect("RPC_URL must be set in .env file");

    let pool = Arc::new(create_pool());

    let txs = match fetch_transactions(pool.clone()).await {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error fetching transactions: {}", e);
            return;
        }
    };

    println!("Starting to analyze {} transactions", txs.len());

    let semaphore = Arc::new(Semaphore::new(64));

    stream::iter(txs)
        .for_each_concurrent(Some(64), |tx| {
            let semaphore = Arc::clone(&semaphore);
            let pool_clone = pool.clone();
            async move {
                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        eprintln!("Error acquiring semaphore: {}", e);
                        return;
                    }
                };

                let tx_info = match get_transaction_info(&tx, &rpc_url).await {
                    Ok(logs) => logs,
                    Err(e) => {
                        eprintln!("Error getting transction info {}: {}", tx, e);
                        return;
                    }
                };

                println!("Tx info {:?}", &tx_info);

                if let Err(e) =
                    insert_into_db(pool_clone, &tx, tx_info.gas_price, &tx_info.data_prefix).await
                {
                    eprintln!("Error inserting data into database for {}: {}", tx, e);
                    return;
                }

                println!("Data inserted for {}", tx);
            }
        })
        .await;
}
