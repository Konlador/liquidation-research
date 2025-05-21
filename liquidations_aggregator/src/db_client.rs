use crate::big_num::*;
use crate::log_parsing::*;
use deadpool_postgres::Runtime;
use deadpool_postgres::{Config, Pool};
use serde_json::to_string;
use std::error::Error as StdError;
use std::sync::Arc;
use std::time::Duration;

// const PG_CONNECTION_STRING: &str = "host=localhost dbname=discovery_manager user=postgres password=root options='-c search_path=bsc,common,public'";

pub struct LiquidationData {
    pub transaction_hash: String,
    pub block_number: i64,
    pub v_token: String,
    pub borrower: String,
    pub repay_amount: U256,
    pub v_token_collateral: String,
    pub seize_tokens: U256,
    pub gas_price: U256,
}

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

// async fn get_db_connection() -> Result<Client, Box<dyn StdError>> {
//     let (client, connection) =
//         tokio_postgres::connect(PG_CONNECTION_STRING, tokio_postgres::NoTls).await?;
//     tokio::spawn(async move {
//         if let Err(e) = connection.await {
//             eprintln!("Connection error: {}", e);
//         }
//     });
//     Ok(client)
// }

pub(crate) async fn fetch_liquidation_data(
    pool: Arc<Pool>,
) -> Result<Vec<LiquidationData>, Box<dyn StdError>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            "WITH venus_core_v_tokens(v_token) AS (
    VALUES
('0x08ceb3f4a7ed3500ca0982bcd0fc7816688084c3'), -- tusdold (deprecated)
('0x151b1e2635a717bcdc836ecd6fbb62b674fe3e1d'), -- vXVS
('0x1610bc33319e9398de5f57b33a5b184c806ad217'), -- dot
('0x26da28954763b92139ed49283625cecaf52c6f94'), -- aave
('0x27ff564707786720c71a2e5c1490a63266683612'), -- uni
('0x2ff3d0f6990a40261c66e1ff2017acbc282eb6d0'), -- sxp (deprecated)
('0x334b3ecb4dca3593bccc3c7ebd1a1c1d1780fbf1'), -- dai
('0x4bd17003473389a42daf6a0a729f6fdb328bbbd7'), -- vai (stablecoin)
('0x57a5297f2cb2c0aac9d554660acd6d385ab50c6b'), -- ltc
('0x5c9476fcd6a4f9a3654139721c949c2233bbbbc8'), -- matic
('0x5f0388ebc2b94fa8e123f404b79ccf5f40b29176'), -- bch
('0x61edcfe8dd6ba3c891cb9bec2dc7657b3b422e93'), -- trxold (deprecated)
('0x650b940a1033b8a1b1873f78730fcfc73ec11f1f'), -- link
('0x6cfdec747f37daf3b87a35a1d9c8ad3063a1a8a0'), -- wbeth
('0x78366446547d062f45b4c0f320cdaa6d710d87bb'), -- ust (maybe deprecated)
('0x86ac3974e2bd0d60825230fa6f355ff11409df5c'), -- cake
('0x882c173bc7ff3b7786ca16dfed3dfffb9ee7847b'), -- btcb
('0x95c78222b3d6e262426483d42cfa53685a67ab9d'), -- busd (deprecated)
('0x972207a639cc1b374b893cc33fa251b55ceb7c07'), -- beth
('0x9a0af7fdb2065ce470d72664de73cae409da28ec'), -- ada
('0xa07c5b74c9b40447a954e1466938b865b6bbea36'), -- bnb
('0xb248a295732e0225acd3337607cc01068e3b9c10'), -- xrp
('0xb91a659e88b51474767cd97ef3196a3e7cedd2c8'), -- luna (maybe deprecated)
('0xbf762cd5991ca1dcddac9ae5c638f5b5dc3bee6e'), -- tusd
('0xc4ef4229fec74ccfe17b2bdef7715fac740ba0ba'), -- fdusd
('0xc5d3466aa484b040ee977073fcf337f2c00071c1'), -- trx
('0xebd0070237a0713e8d94fef1b728d3d993d290ef'), -- can (maybe deprecated)
('0xec3422ef92b2fb59e84c8b02ba73f1fe84ed8d71'), -- doge
('0xeca88125a5adbe82614ffc12d0db554e2e2867c8'), -- usdc
('0xf508fcd89b8bd15579dc79a6827cb4686a3592c8'), -- eth
('0xf91d58b5ae142dacc749f58a49fcbac340cb0343'), -- fil
('0xfd5840cd36d94d7229439859c0112a4185bc0255'), -- usdt
('0xf841cb62c19fCd4fF5CD0AaB5939f3140BaaC3Ea'), -- solvbtc
('0x4d41a36D04D97785bcEA57b057C412b278e6Edcc') -- twt
),
special_txs(transaction_hash) AS (
VALUES
('0x7c97317afe5911e704bd684e8b3fe472d7b8703b54321ab564be2bbeacdb0f5f'), --36 changed config
('0xc81fa724698490d096b04cccb080195517f4df5cfa56121cbee895d05ad0de53'), --37 changed config
('0xb18543cd79c90ef2ca1e463aaf3760e6e4e731b7fa64a86e6f2538de392d49df'),-- 223 changed busd CF TO 0
('0xfdb201f22c08b7a589a758f7836d7b01b020548420da0bc9cd6cd459f24f94ab'),-- accrue other v tokens
('0x951fc0819fef15e65b0141e2b79a35b128aab37ba1a1cf8fa501cfe27a16dbb3'),-- accrue other v tokens
('0xa6cf33bc689c354c8a7cfeea4a0144a87d34adfcb7ae2a461590e6e4549c54e3'),-- accrue other v tokens
('0xe55532f21647763df2827388bb7b0cdb3b72333b620bcc6837a4dcc84e81f16b'),-- accrue other v tokens
('0x71369ebda138b5883900cdcd0f8f9dff33fd8c32b2223cae73f28995deb371fe'),-- accrue other v tokens
('0xea0a5757f71f991082761a3e43abc4d2da11de073910b2792f64efa0d5facf89'),-- accrue other v tokens
('0x6ef043928c8496388f2708e6ede756828ff2e69890ccd38ef4e347a85ed48ee7'),-- accrue other v tokens
('0x6b4b388983fe488eff5984fafb469a176cc0e0a7a081ea44a85af64ef9056f80'),-- accrue other v tokens
('0x0a39e9b42736076024c297aef1c2c3b6d1e955b5306682fc83acb8720a01184e'),-- accrue other v tokens
('0xfc3dd1122d0ae998c3ba8c71e3b11b459875e92eda7033cc4307f2e3cb256a8f'),-- accrue other v tokens
('0x40ecc8d36fe4aedd4806687a67c0ce7930248df85265bd5c2ab395920f7b35f5'),-- accrue other v tokens
('0x13b96225efb4ea4bf029605a2ac1f7a00e2e93a5192d8e38451e86f349ae0b83'),-- accrue other v tokens
('0x2337d53555f6dfc18e49727892db88bb535bed648daf44b31e74aa741a3fb6c4'),-- accrue other v tokens
('0xdf0998778ba7bdfbd8b08e18d60bde89d780d9d55da847a04f7d4dd563e17447'),-- accrue other v tokens
('0xaca49724efc301d943cf968382ca0cfebbe19bbc0f0557f0e9517b5af92165ef'),-- accrue other v tokens
('0x60cb3d5b5e3917834e72ec90b6d2ea9a51d0b3be82e457f3e308d8879a0600b1'),-- accrue other v tokens
('0x82d7a56112d99fe38832abe5830e8c87ff891fecd92a748f4ca1595b018daf6a'),-- accrue other v tokens
('0x351f43cbe363c911cc7643e120e6535cad12ad86d53758171eeebc04e7442bc0'),-- accrue other v tokens
('0xf2b81a8bacaf2403d3afda93d45ef4f3039469c8fb1c2a82f32e614308cf58c0'),-- accrue other v tokens
('0x3f4aebf0d58587eb4d135140a8f90b2ce84478262dedd4a9bc008041670a42e2'),-- accrue other v tokens
('0x23447bf3282eb4e7acec2d57879c7c788167e135cd909d1782fea75119e5868a'),-- accrue other v tokens
('0x03740b129ea220086300d2d104414ee7b861076114967706d28ce1b6f37b192c'),-- accrue other v tokens
('0x53edc1b6764deeef8f273699343b0f5b41dab4c7542e4a6ba9aad6ce0fe6a869'),-- accrue other v tokens
('0xfa9e6926cbff2db06ff60e1e7a6c10ee390b8f4f0519fb967071803cc1908c9c'),-- accrue other v tokens
('0xc7b3e71a9e4924d5561fa742c3adb27d570ad65abdc1ac4117def93f55eef175'),-- accrue other v tokens
('0x143101f41381b36ba5dbc5631ddfbd4768a5729dcb992a2d1e48714154bb9ee5'),-- accrue other v tokens
('0x36fa16c6e77100191584a2ed69233dc146870f7ed2fc07c61e06cc782642aa19'),-- accrue other v tokens
('0x84efcbb93d607867a62a1b2308c61eaead13527def3641d811210e2659a60d1f'),-- accrue other v tokens
('0xf3bd6a6c3a92ea0a8bc6e3e736efe2a5f8581063c21a2a52577783c1804d9d77'),-- accrue other v tokens
('0x87640bc97f041012ace33780ac0c138b5e1692660a17561baa7e0b90d3226704'),-- accrue other v tokens
('0x830b5132502a7d558869611957b73149d8e645c6ad65b3d060a73db06689c5e7'),-- broken simulation
('0xa517e3cf2c4d3cd6eefca063a686e424d2cae02b295dc42e51262529007a08e8'),-- takes too long
--('0x92d1f0c59df5bec1b498b63780071b4d828fb14e24b99eeec4f042dad78fa618'),-- claim xvs
--('0xa24a5ea2084499e472260c50e5e79bbe375265c9f60fdf5885fb97676db178b1'),-- claim xvs
--('0xfbe3aa2fb537cb1e41a8be2d30f5131752b27d3df282d6b6a684d3ad8ced6b4f'),-- claim xvs
--('0xd47b29d65ce9f9035f976486feef588eb8a6a052b067c001a709eb1b4d2a98ac'),-- claim xvs
--('0x05765bb515cd90f523bc2e87427d07c40e4c25f594b5b0086980a461d0baa308'),-- claim xvs
--('0x6c34de7bfa3cc43b18640932ff5e9212e1075fe36ff043593747637f97635d23'),-- claim xvs
--('0x6c34de7bfa3cc43b18640932ff5e9212e1075fe36ff043593747637f97635d23'),-- claim xvs
--('0x801001726f7c0c2434a8ea1680213ebfd5201094087c94d7dac44b7860555f1c'),-- claim xvs
--('0x211b0a07e5804e27bb84f17f662ca6b00ab8a8df70277236a179a63b190d7c02'),-- claim xvs
--('0x5277c7dd83bfeb258f9bdb809989cb95eaf4170115e23b9cd6af590f4e9b6b14'),-- claim xvs
--('0xbb198dec733c669696081d7785ca63bd5e959d70fe954754987829faddbc457f'),-- claim xvs
--('0x779c09e57e678c8462c3c73fade305dca5dfbbcb76a2d62bf496155b445d2480'),-- claim xvs
--('0xbf201d382496d56a563e9e88f2fa0bda9afe0f939ac31a54856c140d3e2e6947'),-- claim xvs
--('0x44cda940f0834d9996726876a1f2a540fe876c48b547596e32cc2fcf6f10e645'),-- claim xvs
--('0x703db59c4d471bb2b12c1416245d42ed846759c0fe2cc9235fe3e29454462e67'),-- claim xvs
--('0x9f76985b8a42795c4b8ea0ae3e7ff7c1286c329b1571df5a05f573d6466f1129'),-- claim xvs
--('0x362fcb1947ca7f21b2cb01ef6e307b271f2bc7b044253bfaca864e172905a0f2'),-- claim xvs
--('0x0aefc5c5770cc301a571eadbdce07623f1d58620757dd80743bdd5db76361d7c'),-- claim xvs
--('0x40310334862593d5930026623cd47ac475af98478089d8711f38542b1c64968c'),-- claim xvs
('')
),
liquidations_to_test AS (
SELECT transaction_hash, block_number, v_token, borrower, repay_amount::TEXT, v_token_collateral, seize_tokens::TEXT, gas_price::TEXT,
        ROW_NUMBER() OVER (PARTITION BY borrower ORDER BY block_number ASC, transaction_index ASC) AS row_num, vlt IS NOT NULL AS is_tested
FROM
    bsc.venus_liquidations vl
    LEFT JOIN bsc.venus_liquidation_tests vlt USING(transaction_hash)
WHERE TRUE
    AND v_token IN (SELECT * FROM venus_core_v_tokens) -- the repay vtoken is from venus protocol
    AND transaction_hash NOT IN (SELECT * FROM special_txs) -- not in manual ban list
    AND (block_number >= 31302048 OR v_token_collateral <> '0x151b1e2635a717bcdc836ecd6fbb62b674fe3e1d') -- not handling istanbul xvs collaterals because the claim venus rewards
    AND v_token <> '0x95c78222b3d6e262426483d42cfa53685a67ab9d' -- not vbusd
    AND borrower <> '0x489a8756c18c0b8b24ec2a2b9ff3d4d447f79bec' -- bnb bridge exploiter
    --AND block_number < 31302048 -- istanbul
    --AND block_number >= 31302048 AND block_number < 35490444 -- berlin
    --AND block_number >= 35490444 AND block_number < 39769787 -- shanghai
    AND block_number >= 39769787 -- cancun
    --AND block_number < 32929228
    --AND v_token_collateral <> '0x151b1e2635a717bcdc836ecd6fbb62b674fe3e1d' -- not handling xvs collateral because claim venus rewards
    --AND transaction_hash = '0x8d286fa28b0eb4d4d4e1a8cdaf078190f207921f5d1a5f198de56f5995e2c606'
ORDER BY random()
)
SELECT * 
FROM liquidations_to_test
WHERE TRUE
AND row_num = 1
AND is_tested = false
",
            &[],
        )
        .await?;

    let liquidation_data: Vec<LiquidationData> = rows
        .into_iter()
        .map(|row| {
            let repay_amount_str: String = row.get(4);
            let seize_tokens_str: String = row.get(6);
            let gas_price_str: String = row.get(7);

            let repay_amount = U256::from_dec_str(&repay_amount_str).unwrap();
            let seize_tokens = U256::from_dec_str(&seize_tokens_str).unwrap();
            let gas_price = U256::from_dec_str(&gas_price_str).unwrap();

            LiquidationData {
                transaction_hash: row.get(0),
                block_number: row.get(1),
                v_token: row.get(2),
                borrower: row.get(3),
                repay_amount,
                v_token_collateral: row.get(5),
                seize_tokens,
                gas_price,
            }
        })
        .collect();

    Ok(liquidation_data)
}

pub(crate) async fn insert_with_retries(
    pool: Arc<Pool>,
    transaction_hash: &str,
    parsed_data: &LiquidationTestResults,
) -> Result<(), Box<dyn StdError>> {
    // println!("Starting to insert data for {}", transaction_hash);
    for attempt in 1..=3 {
        match insert_into_db(pool.clone(), transaction_hash, parsed_data).await {
            Ok(_) => return Ok(()),
            Err(e) if attempt < 3 => {
                eprintln!(
                    "Attempt {}/3 failed: {}. Retrying for {}",
                    attempt, e, transaction_hash
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err("Failed to insert data after 3 attempts".into())
}

pub(crate) async fn insert_into_db(
    pool: Arc<Pool>,
    transaction_hash: &str,
    parsed_data: &LiquidationTestResults,
) -> Result<(), Box<dyn StdError>> {
    let client = pool.get().await?;

    let query = format!(
        "INSERT INTO venus_liquidation_tests (
            transaction_hash, repeat, up_to_close_factor, drain, large_borrow, drain_same_token, largest_cf_first, smallest_cf_first
        ) VALUES ('{}', '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb)
        ON CONFLICT (transaction_hash)
        DO UPDATE SET 
            repeat = EXCLUDED.repeat,
            up_to_close_factor = EXCLUDED.up_to_close_factor,
            drain = EXCLUDED.drain",
        transaction_hash,
        to_string(&parsed_data.repeat).unwrap(),
        to_string(&parsed_data.up_to_close_factor).unwrap(),
        to_string(&parsed_data.drain).unwrap(),
        to_string(&parsed_data.large_borrow).unwrap(),
        to_string(&parsed_data.drain_same_token).unwrap(),
        to_string(&parsed_data.largest_cf_first).unwrap(),
        to_string(&parsed_data.smallest_cf_first).unwrap(),
    );

    client.execute(&query, &[]).await?;

    Ok(())
}
