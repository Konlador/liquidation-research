use deadpool_postgres::Runtime;
use deadpool_postgres::{Config, Pool};
use serde_json::to_string;
use std::error::Error as StdError;
use std::sync::Arc;
use std::time::Duration;

pub(crate) fn create_pool() -> Pool {
    let mut cfg = Config::new();
    cfg.dbname = Some("discovery_manager".to_string());
    cfg.host = Some("localhost".to_string());
    cfg.user = Some("postgres".to_string());
    cfg.password = Some("root".to_string());
    cfg.options = Some("-c search_path=bsc,common,public".to_string());
    cfg.keepalives_idle = Some(Duration::from_secs(10));
    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
        .unwrap();
    pool
}

pub(crate) async fn fetch_transactions(pool: Arc<Pool>) -> Result<Vec<String>, Box<dyn StdError>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            "SELECT DISTINCT transaction_hash FROM bsc.venus_liquidations vl WHERE gas_price IS NULL",
            &[],
        )
        .await?;

    let txs: Vec<String> = rows.into_iter().map(|row| row.get(0)).collect();

    Ok(txs)
}

pub(crate) async fn insert_into_db(
    pool: Arc<Pool>,
    transaction_hash: &str,
    gas_price: u64,
    data_prefix: &str,
) -> Result<(), Box<dyn StdError>> {
    let client = pool.get().await?;

    let query = format!(
        "
UPDATE bsc.venus_liquidations
SET
    gas_price = {},
    data_prefix = '{}'
WHERE transaction_hash = '{}';",
        gas_price as i64, data_prefix, transaction_hash
    );

    client.execute(&query, &[]).await?;

    Ok(())
}
