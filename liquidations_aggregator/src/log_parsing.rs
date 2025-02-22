use crate::big_num::*;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LiquidationCall {
    pub repay_amount: U256,
    pub collateral_v_token_gained: U256,
    pub gas_used_for_liquidation: u64,
}

#[derive(Debug, Serialize, Default)]
pub struct SinglePairLiquidationResult {
    pub incentive: U256,
    pub gas_used_for_approve: Option<u64>,
    pub liquidation_calls: Vec<LiquidationCall>,
    pub repay_amount_total: U256,
    pub collateral_v_token_gained_total: U256,
    pub repay_token_price: U256,
    pub collateral_token_price: U256,
    pub chain_coin_price: U256,
    pub capped_by_collateral: Option<bool>,
    pub redeem: RedeemResult,
}

#[derive(Debug, Serialize, Default)]
pub struct RedeemResult {
    pub collateral_exchange_rate: U256,
    pub cash_in_v_token: U256,
    pub enough_cash: bool,
    pub gas_used_for_redeem: u64,
    pub collateral_underlying_seized: U256,
}

#[derive(Debug, Serialize)]
pub struct LiquidationTestResult<T> {
    pub parsed: T,
    pub raw: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LiquidationTestResults {
    pub repeat: LiquidationTestResult<SinglePairLiquidationResult>,
    pub up_to_close_factor: LiquidationTestResult<SinglePairLiquidationResult>,
    pub drain: LiquidationTestResult<SinglePairLiquidationResult>,
}

fn parse_u256(value: &str) -> U256 {
    U256::from_dec_str(value).expect("Failed to parse U256")
}

fn parse_u64(value: &str) -> u64 {
    value.parse::<u64>().expect("Failed to parse u64")
}

fn parse_bool(value: &str) -> bool {
    value.parse::<bool>().expect("Failed to parse bool")
}

// Function to find a specific field in the log and return the associated value
fn find_field<'a>(lines: &'a [&str], field_name: &str) -> Option<&'a str> {
    lines.iter().find_map(|line| {
        if line.contains(field_name) {
            line.split_whitespace().last()
        } else {
            None
        }
    })
}

// Function to parse a single liquidation call
fn parse_liquidation_call(lines: &[&str], prefix: Option<&str>) -> LiquidationCall {
    let prefix = prefix.unwrap_or("");

    let repay_amount = parse_u256(find_field(lines, &format!("{}repayAmount", prefix)).unwrap());
    let collateral_v_token_gained =
        parse_u256(find_field(lines, &format!("{}collateralVTokenGained", prefix)).unwrap());
    let gas_used_for_liquidation =
        parse_u64(find_field(lines, &format!("{}gasUsedForLiquidation", prefix)).unwrap());

    LiquidationCall {
        repay_amount,
        collateral_v_token_gained,
        gas_used_for_liquidation,
    }
}

fn parse_redeem_result(lines: &[&str]) -> RedeemResult {
    let collateral_exchange_rate = parse_u256(find_field(lines, "collateralExchangeRate").unwrap());
    let cash_in_v_token = parse_u256(find_field(lines, "cashInVToken").unwrap());
    let enough_cash = parse_bool(find_field(lines, "enoughCash").unwrap());
    let gas_used_for_redeem = parse_u64(find_field(lines, "gasUsedForRedeem").unwrap());
    let collateral_underlying_seized =
        parse_u256(find_field(lines, "collateralUnderlyingSeized").unwrap());

    RedeemResult {
        collateral_exchange_rate,
        cash_in_v_token,
        enough_cash,
        gas_used_for_redeem,
        collateral_underlying_seized,
    }
}

// Parse a test result for a block like `repeatLiquidation` or `upToCloseFactorLiquidation`
fn parse_single_pair_liquidation_result(lines: &[&str]) -> SinglePairLiquidationResult {
    let incentive = parse_u256(find_field(lines, "incentive").unwrap());

    let gas_used_for_approve = find_field(lines, "gasUsedForApprove").map(|value| parse_u64(value));

    let liquidation_call = parse_liquidation_call(&lines, None);
    let repay_token_price = parse_u256(find_field(lines, "repayTokenPrice").unwrap());
    let collateral_token_price = parse_u256(find_field(lines, "collateralTokenPrice").unwrap());
    let chain_coin_price = parse_u256(find_field(lines, "chainCoinPrice").unwrap());
    let capped_by_collateral =
        find_field(lines, "cappedByCollateral").map(|value| parse_bool(value));
    let redeem = parse_redeem_result(&lines);

    // For these cases, total equals the value from the single liquidation call
    let repay_amount_total = liquidation_call.repay_amount;
    let collateral_v_token_gained_total = liquidation_call.collateral_v_token_gained;

    SinglePairLiquidationResult {
        incentive,
        gas_used_for_approve,
        liquidation_calls: vec![liquidation_call],
        repay_amount_total,
        collateral_v_token_gained_total,
        repay_token_price,
        collateral_token_price,
        chain_coin_price,
        capped_by_collateral,
        redeem,
    }
}

// Parse the `drainLiquidation` case where there are multiple liquidation calls
fn parse_drain_liquidation_result(lines: &[&str]) -> SinglePairLiquidationResult {
    let incentive = parse_u256(find_field(lines, "incentive").unwrap());

    let gas_used_for_approve = find_field(lines, "gasUsedForApprove").map(|value| parse_u64(value));

    let mut liquidation_calls = Vec::new();
    let mut i = 1;

    // Parse all individual liquidation calls with prefixes (1, 2, etc.)
    while let Some(_) = find_field(lines, &format!("{} repayAmount", i)) {
        liquidation_calls.push(parse_liquidation_call(&lines, Some(&format!("{} ", i))));
        i += 1;
    }

    // Parse the remaining fields (totals)
    let repay_amount_total = parse_u256(find_field(lines, "repayAmountTotal").unwrap());
    let collateral_v_token_gained_total =
        parse_u256(find_field(lines, "collateralVTokenGainedTotal").unwrap());
    let repay_token_price = parse_u256(find_field(lines, "repayTokenPrice").unwrap());
    let collateral_token_price = parse_u256(find_field(lines, "collateralTokenPrice").unwrap());
    let chain_coin_price = parse_u256(find_field(lines, "chainCoinPrice").unwrap());
    let capped_by_collateral =
        find_field(lines, "cappedByCollateral").map(|value| parse_bool(value));
    let redeem = parse_redeem_result(&lines);

    SinglePairLiquidationResult {
        incentive,
        gas_used_for_approve,
        liquidation_calls,
        repay_amount_total,
        collateral_v_token_gained_total,
        repay_token_price,
        collateral_token_price,
        chain_coin_price,
        capped_by_collateral,
        redeem,
    }
}

// Main log parser function
pub fn parse_logs(logs: &str) -> LiquidationTestResults {
    let mut repeat_raw: Vec<String> = Vec::new();
    let mut up_to_close_factor_raw: Vec<String> = Vec::new();
    let mut drain_raw: Vec<String> = Vec::new();

    let mut repeat_parsed: Option<SinglePairLiquidationResult> = None;
    let mut up_to_close_factor_parsed: Option<SinglePairLiquidationResult> = None;
    let mut drain_parsed: Option<SinglePairLiquidationResult> = None;

    let lines: Vec<&str> = logs.lines().collect();

    let mut current_test = None;

    for &line in &lines {
        if line.contains("Tests case: repeatLiquidation") {
            current_test = Some("repeatLiquidation");
        } else if line.contains("Tests case: upToCloseFactorLiquidation") {
            current_test = Some("upToCloseFactorLiquidation");
        } else if line.contains("Tests case: drainLiquidation") {
            current_test = Some("drainLiquidation");
        } else if line.contains("Tests case end") {
            match current_test {
                Some("repeatLiquidation") => {
                    repeat_parsed = Some(parse_single_pair_liquidation_result(
                        &repeat_raw.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
                    ));
                }
                Some("upToCloseFactorLiquidation") => {
                    up_to_close_factor_parsed = Some(parse_single_pair_liquidation_result(
                        &up_to_close_factor_raw
                            .iter()
                            .map(|s| s.as_ref())
                            .collect::<Vec<_>>(),
                    ));
                }
                Some("drainLiquidation") => {
                    drain_parsed = Some(parse_drain_liquidation_result(
                        &drain_raw.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
                    ));
                }
                _ => {}
            }
            current_test = None;
        } else {
            // Append raw data based on the current test being processed
            match current_test {
                Some("repeatLiquidation") => {
                    repeat_raw.push(line.to_string());
                }
                Some("upToCloseFactorLiquidation") => {
                    up_to_close_factor_raw.push(line.to_string());
                }
                Some("drainLiquidation") => {
                    drain_raw.push(line.to_string());
                }
                _ => {}
            }
        }
    }

    LiquidationTestResults {
        repeat: LiquidationTestResult {
            raw: repeat_raw,
            parsed: repeat_parsed.unwrap(),
        },
        up_to_close_factor: LiquidationTestResult {
            raw: up_to_close_factor_raw,
            parsed: up_to_close_factor_parsed.unwrap(),
        },
        drain: LiquidationTestResult {
            raw: drain_raw,
            parsed: drain_parsed.unwrap_or_default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_logs() {
        let logs = r#"
  Tests case: repeatLiquidation
  incentive 1100000000000000000
  gasUsedForApprove 24263
  repayAmount 16642946542591384000935
  collateralVTokenGained 3437968650492
  gasUsedForLiquidation 595377
  collateralExchangeRate 200000634586835700427250667
  cashInVToken 600271447649471624195856
  enoughCash true
  gasUsedForRedeem 62355
  collateralUnderlyingSeized 687595911788047152986
  repayTokenPrice 1000000000000000000
  collateralTokenPrice 26625000000000000000
  chainCoinPrice 26625000000000000000
  Tests case end
  
  Tests case: upToCloseFactorLiquidation
  incentive 1100000000000000000
  gasUsedForApprove 24263
  repayAmount 16642946542590932675453
  cappedByCollateral false
  collateralVTokenGained 3437968650492
  gasUsedForLiquidation 595382
  collateralExchangeRate 200000634586835700427250667
  cashInVToken 600271447649471624195856
  enoughCash true
  gasUsedForRedeem 87355
  collateralUnderlyingSeized 687595911788047152986
  repayTokenPrice 1000000000000000000
  collateralTokenPrice 26625000000000000000
  chainCoinPrice 26625000000000000000
  Tests case end
  
  Tests case: drainLiquidation
  incentive 1100000000000000000
  gasUsedForApprove 24263
  1 repayAmount 122986808185542410628
  1 collateralVTokenGained 25405644961
  1 gasUsedForLiquidation 595390
  Last liquidation needed true
  2 repayAmount 16581453138495741007908
  2 collateralVTokenGained 3425265828011
  2 gasUsedForLiquidation 354830
  cappedByCollateral false
  repayAmountTotal 16704439946681283418536
  collateralVTokenGainedTotal 3450671472972
  collateralExchangeRate 200000634586835700427250667
  cashInVToken 600271447649471624195856
  enoughCash true
  gasUsedForRedeem 87355
  collateralUnderlyingSeized 690136484345091075033
  repayTokenPrice 1000000000000000000
  collateralTokenPrice 26625000000000000000
  chainCoinPrice 26625000000000000000
  Tests case end
        "#;

        let result = parse_logs(logs);

        // Assertions for repeatLiquidation
        let repeat = &result.repeat.parsed;
        assert_eq!(
            repeat.repay_amount_total,
            U256::from_dec_str("16642946542591384000935").unwrap()
        );
        assert_eq!(
            repeat.collateral_v_token_gained_total,
            U256::from_dec_str("3437968650492").unwrap()
        );
        assert_eq!(repeat.redeem.gas_used_for_redeem, 62355);
        assert_eq!(
            repeat.repay_token_price,
            U256::from_dec_str("1000000000000000000").unwrap()
        );
        assert_eq!(repeat.capped_by_collateral, None);

        // Assertions for upToCloseFactorLiquidation
        let up_to_close_factor = &result.up_to_close_factor.parsed;
        assert_eq!(
            up_to_close_factor.repay_amount_total,
            U256::from_dec_str("16642946542590932675453").unwrap()
        );
        assert_eq!(
            up_to_close_factor.collateral_v_token_gained_total,
            U256::from_dec_str("3437968650492").unwrap()
        );
        assert_eq!(up_to_close_factor.redeem.gas_used_for_redeem, 87355);
        assert_eq!(
            up_to_close_factor.collateral_token_price,
            U256::from_dec_str("26625000000000000000").unwrap()
        );
        assert_eq!(up_to_close_factor.capped_by_collateral, Some(false));

        // Assertions for drainLiquidation
        let drain = &result.drain.parsed;
        assert_eq!(
            drain.repay_amount_total,
            U256::from_dec_str("16704439946681283418536").unwrap()
        );
        assert_eq!(
            drain.collateral_v_token_gained_total,
            U256::from_dec_str("3450671472972").unwrap()
        );
        assert_eq!(drain.redeem.gas_used_for_redeem, 87355);
        assert_eq!(
            drain.chain_coin_price,
            U256::from_dec_str("26625000000000000000").unwrap()
        );
        assert_eq!(drain.capped_by_collateral, Some(false));

        // Ensure there are multiple liquidation calls in the drainLiquidation case
        assert_eq!(drain.liquidation_calls.len(), 2);
        assert_eq!(
            drain.liquidation_calls[0].repay_amount,
            U256::from_dec_str("122986808185542410628").unwrap()
        );
        assert_eq!(
            drain.liquidation_calls[1].repay_amount,
            U256::from_dec_str("16581453138495741007908").unwrap()
        );
    }

    #[test]
    fn test_parse_logs_1() {
        let logs = r#"
  Tests case: repeatLiquidation
  gasUsedForApprove 24263
  repayAmount 5000000000000000000
  collateralVTokenGained 84433890
  gasUsedForLiquidation 2142402
  collateralExchangeRate 217219347202050066643482877
  gasUsedForRedeem 70888
  collateralUnderlyingSeized 18340674467529703
  repayTokenPrice 999300000000000000
  collateralTokenPrice 299670000000000000000
  chainCoinPrice 299670000000000000000
  Tests case end
  
  Tests case: upToCloseFactorLiquidation
  gasUsedForApprove 24263
  repayAmount 15408330375152698594
  collateralVTokenGained 260197054
  gasUsedForLiquidation 2142405
  collateralExchangeRate 217219347202050066643482877
  gasUsedForRedeem 95884
  collateralUnderlyingSeized 56519834213776570
  repayTokenPrice 999300000000000000
  collateralTokenPrice 299670000000000000000
  chainCoinPrice 299670000000000000000
  Tests case end
  
  Tests case: drainLiquidation
  gasUsedForApprove 24263
  Max repayAmount will keep the borrower liquidatable!!!
  1 repayAmount 15408330375152698594
  1 collateralVTokenGained 260197054
  1 gasUsedForLiquidation 2142408
  Max repayAmount will keep the borrower liquidatable!!!
  2 repayAmount 7704165187576349297
  2 collateralVTokenGained 130098527
  2 gasUsedForLiquidation 1977300
  Max repayAmount will keep the borrower liquidatable!!!
  3 repayAmount 3852082593788174648
  3 collateralVTokenGained 65049263
  3 gasUsedForLiquidation 1977301
  Max repayAmount will keep the borrower liquidatable!!!
  4 repayAmount 1926041296894087324
  4 collateralVTokenGained 32524631
  4 gasUsedForLiquidation 1977302
  Max repayAmount will keep the borrower liquidatable!!!
  5 repayAmount 963020648447043662
  5 collateralVTokenGained 16262315
  5 gasUsedForLiquidation 1977302
  Max repayAmount will keep the borrower liquidatable!!!
  6 repayAmount 481510324223521831
  6 collateralVTokenGained 8131157
  6 gasUsedForLiquidation 1977303
  Max repayAmount will keep the borrower liquidatable!!!
  7 repayAmount 240755162111760916
  7 collateralVTokenGained 4065578
  7 gasUsedForLiquidation 1977303
  Max repayAmount will keep the borrower liquidatable!!!
  8 repayAmount 120377581055880458
  8 collateralVTokenGained 2032789
  8 gasUsedForLiquidation 1977304
  Max repayAmount will keep the borrower liquidatable!!!
  9 repayAmount 60188790527940229
  9 collateralVTokenGained 1016394
  9 gasUsedForLiquidation 1977305
  Max repayAmount will keep the borrower liquidatable!!!
  10 repayAmount 30094395263970114
  10 collateralVTokenGained 508197
  10 gasUsedForLiquidation 1977305
  Max repayAmount will keep the borrower liquidatable!!!
  11 repayAmount 15047197631985057
  11 collateralVTokenGained 254098
  11 gasUsedForLiquidation 1977306
  Max repayAmount will keep the borrower liquidatable!!!
  12 repayAmount 7523598815992529
  12 collateralVTokenGained 127049
  12 gasUsedForLiquidation 1977307
  Max repayAmount will keep the borrower liquidatable!!!
  13 repayAmount 3761799407996264
  13 collateralVTokenGained 63524
  13 gasUsedForLiquidation 1977308
  Max repayAmount will keep the borrower liquidatable!!!
  14 repayAmount 1880899703998132
  14 collateralVTokenGained 31762
  14 gasUsedForLiquidation 1977308
  Max repayAmount will keep the borrower liquidatable!!!
  15 repayAmount 940449851999066
  15 collateralVTokenGained 15881
  15 gasUsedForLiquidation 1977309
  Max repayAmount will keep the borrower liquidatable!!!
  16 repayAmount 470224925999533
  16 collateralVTokenGained 7940
  16 gasUsedForLiquidation 1977309
  Max repayAmount will keep the borrower liquidatable!!!
  17 repayAmount 235112462999767
  17 collateralVTokenGained 3970
  17 gasUsedForLiquidation 1977310
  Max repayAmount will keep the borrower liquidatable!!!
  18 repayAmount 117556231499883
  18 collateralVTokenGained 1985
  18 gasUsedForLiquidation 1977310
  Max repayAmount will keep the borrower liquidatable!!!
  19 repayAmount 58778115749942
  19 collateralVTokenGained 992
  19 gasUsedForLiquidation 1977311
  Max repayAmount will keep the borrower liquidatable!!!
  20 repayAmount 29389057874971
  20 collateralVTokenGained 496
  20 gasUsedForLiquidation 1977312
  Max repayAmount will keep the borrower liquidatable!!!
  21 repayAmount 14694528937485
  21 collateralVTokenGained 248
  21 gasUsedForLiquidation 1977312
  Max repayAmount will keep the borrower liquidatable!!!
  22 repayAmount 7347264468743
  22 collateralVTokenGained 124
  22 gasUsedForLiquidation 1977313
  Max repayAmount will keep the borrower liquidatable!!!
  23 repayAmount 3673632234371
  23 collateralVTokenGained 62
  23 gasUsedForLiquidation 1977314
  Max repayAmount will keep the borrower liquidatable!!!
  24 repayAmount 1836816117186
  24 collateralVTokenGained 31
  24 gasUsedForLiquidation 1977314
  Max repayAmount will keep the borrower liquidatable!!!
  25 repayAmount 918408058593
  25 collateralVTokenGained 15
  25 gasUsedForLiquidation 1977314
  Max repayAmount will keep the borrower liquidatable!!!
  26 repayAmount 459204029296
  26 collateralVTokenGained 7
  26 gasUsedForLiquidation 1977315
  Max repayAmount will keep the borrower liquidatable!!!
  27 repayAmount 229602014648
  27 collateralVTokenGained 3
  27 gasUsedForLiquidation 1977316
  Max repayAmount will keep the borrower liquidatable!!!
  28 repayAmount 114801007324
  28 collateralVTokenGained 1
  28 gasUsedForLiquidation 1977316
  Max repayAmount will keep the borrower liquidatable!!!
  29 repayAmount 57400503662
  29 collateralVTokenGained 0
  29 gasUsedForLiquidation 1977317
  30 repayAmount 28700251831
  30 collateralVTokenGained 0
  30 gasUsedForLiquidation 1977317
  repayAmountTotal 30816660721605145356
  collateralVTokenGainedTotal 520394093
  collateralExchangeRate 217219347202050066643482877
  gasUsedForRedeem 95895
  collateralUnderlyingSeized 113039665169262932
  repayTokenPrice 999300000000000000
  collateralTokenPrice 299670000000000000000
  chainCoinPrice 299670000000000000000
  Tests case end
        "#;

        let result = parse_logs(logs);
        println!("{:#?}", result);
    }
}
