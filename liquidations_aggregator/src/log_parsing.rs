use {crate::big_num::*, serde::Serialize, std::collections::HashMap};

#[derive(Debug, Serialize, Clone)]
pub struct StrategyRunReport {
    pub initial_health_factor: String, // Without any accrue
    pub final_health_factor: String,
    pub liquidations: Vec<LiquidationReport>,
    pub assets: Vec<AssetReport>,
    pub gas_usage: GasUsage,
    pub gas_price: U256,
    pub chain_coin_price: U256,
    pub gas_fee_usd: String, // gas_usage.total * gas_price * chain_coin_price
    pub repaid_usd: String,  // sum by asset the sum of repaid * price
    pub seized_usd: String,
    pub profit_usd: String,

    pub raw: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LiquidationReport {
    pub repay_symbol: String,
    pub collateral_symbol: String,
    pub repay_v_token: String,
    pub collateral_v_token: String,
    pub repay_amount: U256,
    pub collateral_v_token_gained: U256,
    pub collateral_underlying_gained: U256, // Includes redeem fees
    pub gas_used: U256,
    pub post_health_factor: String,
    // Derivatives
    pub repaid_usd: String, // repay_amount * price
    pub seized_usd: String, // collateral_underlying_gained * price
}

#[derive(Debug, Serialize, Clone)]
pub struct AssetReport {
    pub initial_data: AssetData,
    pub repaid: U256,
    pub collateral_v_token_gained: U256,
    pub collateral_underlying_gained: U256,
    pub liquidations_participated: U256,
    pub gas_used_to_approve: U256,
    pub gas_used_to_redeem: U256,
}

#[derive(Debug, Serialize, Clone)]
pub struct AssetData {
    pub symbol: String,
    pub v_token: String,
    pub collateral_factor: U256,
    pub collateral_amount: U256,
    pub borrow_amount: U256,
    pub cash: U256,
    pub exchange_rate: U256,
    pub price: U256,
    // Derivatives:
    pub borrow_value_usd: String,     // borrow_amount * price
    pub collateral_value_usd: String, // min(collateral_amount * exchange_rate, cash) * price
    pub is_collateral_capped_by_cash: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct GasUsage {
    pub approves: U256,
    pub liquidations: U256,
    pub redeems: U256,
    pub total: U256,
}

#[derive(Debug, Serialize, Clone)]
pub struct LiquidationTestResults {
    pub repeat: StrategyRunReport,
    pub up_to_close_factor: StrategyRunReport,
    pub drain: StrategyRunReport,
    pub large_borrow: StrategyRunReport,
    pub drain_same_token: StrategyRunReport,
    pub largest_cf_first: StrategyRunReport,
    pub smallest_cf_first: StrategyRunReport,
}

fn extract_str(line: &str) -> String {
    line.split_whitespace().last().unwrap().to_owned()
}

fn extract_u256(line: &str) -> U256 {
    let dec_str = line.split_whitespace().last().unwrap();
    match U256::from_dec_str(dec_str) {
        Ok(value) => value,
        Err(_) => panic!("kek"),
    }
}

pub fn parse_strategy_run_report(lines: Vec<String>) -> StrategyRunReport {
    let mut current_report = StrategyRunReport {
        initial_health_factor: "".to_string(),
        final_health_factor: "".to_string(),
        liquidations: Vec::new(),
        assets: Vec::new(),
        gas_usage: GasUsage {
            approves: U256::zero(),
            liquidations: U256::zero(),
            redeems: U256::zero(),
            total: U256::zero(),
        },
        gas_price: U256::zero(),
        chain_coin_price: U256::zero(),
        gas_fee_usd: "".to_string(),
        repaid_usd: "".to_string(),
        seized_usd: "".to_string(),
        profit_usd: "".to_string(),
        raw: lines.clone(),
    };

    let mut current_asset: Option<AssetReport> = None;
    let mut current_liquidation: Option<LiquidationReport> = None;
    let mut current_gas_usage: Option<GasUsage> = None;

    let mut parsing_strategy = false;

    for line in lines.iter() {
        if line.starts_with("strategyRunReport start") {
            parsing_strategy = true;
        } else if line.starts_with("strategyRunReport end") {
            parsing_strategy = false;
        }

        if !parsing_strategy {
            continue;
        }

        if line.starts_with("asset start") {
            current_asset = Some(AssetReport {
                initial_data: AssetData {
                    symbol: "".to_string(),
                    v_token: "".to_string(),
                    collateral_factor: U256::zero(),
                    collateral_amount: U256::zero(),
                    borrow_amount: U256::zero(),
                    cash: U256::zero(),
                    exchange_rate: U256::zero(),
                    price: U256::zero(),
                    borrow_value_usd: "".to_string(),
                    collateral_value_usd: "".to_string(),
                    is_collateral_capped_by_cash: false,
                },
                repaid: U256::zero(),
                collateral_v_token_gained: U256::zero(),
                collateral_underlying_gained: U256::zero(),
                liquidations_participated: U256::zero(),
                gas_used_to_approve: U256::zero(),
                gas_used_to_redeem: U256::zero(),
            });
        } else if line.starts_with("asset end") {
            if let Some(ref asset) = current_asset {
                current_report.assets.push(asset.clone());
            }
            current_asset = None;
        } else if let Some(ref mut asset) = current_asset {
            if line.starts_with("symbol ") {
                asset.initial_data.symbol = extract_str(line);
            } else if line.starts_with("vtoken ") {
                asset.initial_data.v_token = extract_str(line);
            } else if line.starts_with("collateralFactor ") {
                asset.initial_data.collateral_factor = extract_u256(line);
            } else if line.starts_with("collateralAmount ") {
                asset.initial_data.collateral_amount = extract_u256(line);
            } else if line.starts_with("borrowAmount ") {
                asset.initial_data.borrow_amount = extract_u256(line);
            } else if line.starts_with("cash ") {
                asset.initial_data.cash = extract_u256(line);
            } else if line.starts_with("exchangeRate ") {
                asset.initial_data.exchange_rate = extract_u256(line);
            } else if line.starts_with("price ") {
                asset.initial_data.price = extract_u256(line);
            } else if line.starts_with("borrowValueUsd ") {
                asset.initial_data.borrow_value_usd = extract_str(line);
            } else if line.starts_with("collateralValueUsd ") {
                asset.initial_data.collateral_value_usd = extract_str(line);
            } else if line.starts_with("isCollateralCappedByCash ") {
                asset.initial_data.is_collateral_capped_by_cash = extract_str(line) == "true";
            } else if line.starts_with("repaid ") {
                asset.repaid = extract_u256(line);
            } else if line.starts_with("collateralVTokenGained ") {
                asset.collateral_v_token_gained = extract_u256(line);
            } else if line.starts_with("collateralUnderlyingGained ") {
                asset.collateral_underlying_gained = extract_u256(line);
            } else if line.starts_with("liquidationsParticipated ") {
                asset.liquidations_participated = extract_u256(line);
            } else if line.starts_with("gasUsedToApprove ") {
                asset.gas_used_to_approve = extract_u256(line);
            } else if line.starts_with("gasUsedToRedeem ") {
                asset.gas_used_to_redeem = extract_u256(line);
            }
        }

        // Parsing liquidation sections
        if line.starts_with("liquidation start") {
            current_liquidation = Some(LiquidationReport {
                repay_symbol: "".to_string(),
                collateral_symbol: "".to_string(),
                repay_v_token: "".to_string(),
                collateral_v_token: "".to_string(),
                repay_amount: U256::zero(),
                collateral_v_token_gained: U256::zero(),
                collateral_underlying_gained: U256::zero(),
                gas_used: U256::zero(),
                post_health_factor: "".to_string(),
                repaid_usd: "".to_string(),
                seized_usd: "".to_string(),
            });
        } else if line.starts_with("liquidation end") {
            if let Some(ref liquidation) = current_liquidation {
                current_report.liquidations.push(liquidation.clone());
            }
            current_liquidation = None;
        } else if let Some(ref mut liquidation) = current_liquidation {
            if line.starts_with("repaySymbol ") {
                liquidation.repay_symbol = extract_str(line);
            } else if line.starts_with("collateralSymbol ") {
                liquidation.collateral_symbol = extract_str(line);
            } else if line.starts_with("repayVToken ") {
                liquidation.repay_v_token = extract_str(line);
            } else if line.starts_with("collateralVToken ") {
                liquidation.collateral_v_token = extract_str(line);
            } else if line.starts_with("repayAmount ") {
                liquidation.repay_amount = extract_u256(line);
            } else if line.starts_with("collateralVTokenGained ") {
                liquidation.collateral_v_token_gained = extract_u256(line);
            } else if line.starts_with("collateralUnderlyingGained ") {
                liquidation.collateral_underlying_gained = extract_u256(line);
            } else if line.starts_with("gasUsed ") {
                liquidation.gas_used = extract_u256(line);
            } else if line.starts_with("postHealthFactor ") {
                liquidation.post_health_factor = extract_str(line);
            } else if line.starts_with("repaidUsd ") {
                liquidation.repaid_usd = extract_str(line);
            } else if line.starts_with("seizedUsd ") {
                liquidation.seized_usd = extract_str(line);
            }
        }

        // Parsing gas usage data
        if line.starts_with("gasUsage start") {
            current_gas_usage = Some(GasUsage {
                approves: U256::zero(),
                liquidations: U256::zero(),
                redeems: U256::zero(),
                total: U256::zero(),
            });
        } else if line.starts_with("gasUsage end") {
            if let Some(ref gas_usage) = current_gas_usage {
                current_report.gas_usage = gas_usage.clone();
            }
            current_gas_usage = None;
        } else if let Some(ref mut gas_usage) = current_gas_usage {
            if line.starts_with("approves ") {
                gas_usage.approves = extract_u256(line);
            } else if line.starts_with("liquidations ") {
                gas_usage.liquidations = extract_u256(line);
            } else if line.starts_with("redeems ") {
                gas_usage.redeems = extract_u256(line);
            } else if line.starts_with("total ") {
                gas_usage.total = extract_u256(line);
            }
        }

        // Handle strategy data parsing
        if line.starts_with("initialHealthFactor ") {
            current_report.initial_health_factor = extract_str(line);
        } else if line.starts_with("finalHealthFactor ") {
            current_report.final_health_factor = extract_str(line);
        } else if line.starts_with("gasPrice ") {
            current_report.gas_price = extract_u256(line);
        } else if line.starts_with("chainCoinPrice ") {
            current_report.chain_coin_price = extract_u256(line);
        } else if line.starts_with("gasFeeUsd ") {
            current_report.gas_fee_usd = extract_str(line);
        } else if line.starts_with("repaidUsd ") {
            current_report.repaid_usd = extract_str(line);
        } else if line.starts_with("seizedUsd ") {
            current_report.seized_usd = extract_str(line);
        } else if line.starts_with("profitUsd ") {
            current_report.profit_usd = extract_str(line);
        }
    }

    current_report
}

// Main log parser function
pub fn parse_logs(logs: &str) -> LiquidationTestResults {
    let mut test_case_map: HashMap<String, Vec<String>> = HashMap::new();

    let lines = logs.lines();
    let mut current_test: Option<String> = None;
    let mut current_raw_lines: Vec<String> = Vec::new();

    for line in lines {
        let line = line.trim_start(); // Remove leading whitespace

        if line.starts_with("Tests case: ") {
            // Extract the test case name
            if let Some(test_name) = line.strip_prefix("Tests case: ") {
                current_test = Some(test_name.to_string());
                current_raw_lines.clear();
            }
        } else if line.starts_with("Tests case end") {
            // When "Tests case end" is found, process the current test case
            if let Some(test_name) = &current_test {
                test_case_map.insert(test_name.clone(), current_raw_lines.clone());
                current_test = None;
            }
        } else {
            // Collect raw lines for the current test case
            if let Some(_) = &current_test {
                current_raw_lines.push(line.to_string());
            }
        }
    }

    let mut parsed_reports: HashMap<String, StrategyRunReport> = HashMap::new();
    // Parse the collected raw lines into StrategyRunReport
    for (test_name, raw_lines) in test_case_map {
        let parsed_report = parse_strategy_run_report(raw_lines);

        parsed_reports.insert(test_name, parsed_report);
    }

    LiquidationTestResults {
        repeat: parsed_reports.get("repeatLiquidation").unwrap().clone(),
        up_to_close_factor: parsed_reports
            .get("upToCloseFactorLiquidation")
            .unwrap()
            .clone(),
        drain: parsed_reports.get("drainLiquidation").unwrap().clone(),
        large_borrow: parsed_reports.get("largestBorrow").unwrap().clone(),
        drain_same_token: parsed_reports.get("drainSameToken").unwrap().clone(),
        largest_cf_first: parsed_reports
            .get("largestCollateralFactorFirst")
            .unwrap()
            .clone(),
        smallest_cf_first: parsed_reports
            .get("smallestCollateralFactorFirst")
            .unwrap()
            .clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_logs() {
        let logs = r#"
  incentive 1100000000000000000
  liquidator 0x0000000000000000000000000000000000000000
  treasuryPercent 0
  
  Tests case: repeatLiquidation
  
  strategyRunReport start
  initialHealthFactor 0.888549544472118463
  finalHealthFactor 0.888556383447936982
  asset start 0
  initialData start
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  initialData end
  repaid 960639351957199126528
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 1
  gasUsedToApprove 25175
  gasUsedToRedeem 0
  asset end
  asset start 1
  initialData start
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  initialData end
  repaid 0
  collateralVTokenGained 11802590948156
  collateralUnderlyingGained 2387143349
  liquidationsParticipated 1
  gasUsedToApprove 0
  gasUsedToRedeem 70744
  asset end
  numberOfLiquidations 1
  liquidation start 0
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 960639351957199126528
  collateralVTokenGained 11802590948156
  collateralUnderlyingGained 2387143349
  gasUsed 1362875
  postHealthFactor 0.888556383447936982
  repaidUsd 961.382617836595450636
  seizedUsd 1057.520879410374140000
  liquidation end
  gasUsage start
  approves 25175
  liquidations 1362875
  redeems 70744
  total 1458794
  gasUsage end
  gasPrice 5000000000
  chainCoinPrice 307500000000000000000
  gasFeeUsd 2.242895775000000000
  repaidUsd 961.382617836595450636
  seizedUsd 1057.520879410374140000
  profitUsd 93.895365798778689364
  strategyRunReport end
  Tests case end
  
  Tests case: upToCloseFactorLiquidation
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 25486.863686647855258464 28035.550053096303000000 2.106238162500000000
  
  strategyRunReport start
  initialHealthFactor 0.888549544472118463
  finalHealthFactor 0.888734247444888248
  asset start 0
  initialData start
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  initialData end
  repaid 25467159236203619793758
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 1
  gasUsedToApprove 25196
  gasUsedToRedeem 0
  asset end
  asset start 1
  initialData start
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  initialData end
  repaid 0
  collateralVTokenGained 312894180801642
  collateralUnderlyingGained 63284686050
  liquidationsParticipated 1
  gasUsedToApprove 0
  gasUsedToRedeem 72756
  asset end
  numberOfLiquidations 1
  liquidation start 0
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 25467159236203619793758
  collateralVTokenGained 312894180801642
  collateralUnderlyingGained 63284686050
  gasUsed 1369911
  postHealthFactor 0.888734247444888248
  repaidUsd 25486.863686647855258464
  seizedUsd 28035.550053096303000000
  liquidation end
  gasUsage start
  approves 25196
  liquidations 1369911
  redeems 72756
  total 1467863
  gasUsage end
  gasPrice 5000000000
  chainCoinPrice 307500000000000000000
  gasFeeUsd 2.256839362500000000
  repaidUsd 25486.863686647855258464
  seizedUsd 28035.550053096303000000
  profitUsd 2546.429527085947741536
  strategyRunReport end
  Tests case end
  
  Tests case: drainLiquidation
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 25486.863686647855258464 28035.550053096303000000 2.106291975000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 12743.431843323927629232 14017.775026548151500000 0.754518900000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6371.715921662004542225 7008.887513052572320000 0.754535812500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 3185.857960830961543503 3504.443756526286160000 0.754551187500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 1592.928980415521499361 1752.221878263143080000 0.754569637500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 796.464490207720022071 876.110939131571540000 0.754585012500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 398.232245103900738645 438.055469344282340000 0.754601925000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 199.116122551950369322 219.027734450637740000 0.754618837500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 99.558061275934457052 109.513867003815440000 0.754634212500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 49.779030637967228526 54.756933501907720000 0.754651125000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 24.889515318983614263 27.378466750953860000 0.754668037500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 12.444757659532534740 13.689233153973500000 0.754683412500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6.222378829766267370 6.844616355483320000 0.754698787500000000
  liquidation does not cover gas cost (revenue, cost) 0.622237525717052630 0.754698787500000000
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6.222378829766267370 6.844616355483320000 0.754706475000000000
  liquidation does not cover gas cost (revenue, cost) 0.622237525717052630 0.754706475000000000
  
  strategyRunReport start
  initialHealthFactor 0.888549544472118463
  finalHealthFactor 0.888926995441693728
  asset start 0
  initialData start
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  initialData end
  repaid 50921883336061481947598
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 12
  gasUsedToApprove 25219
  gasUsedToRedeem 0
  asset end
  asset start 1
  initialData start
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  initialData end
  repaid 0
  collateralVTokenGained 625635581241565
  collateralUnderlyingGained 126538471376
  liquidationsParticipated 12
  gasUsedToApprove 0
  gasUsedToRedeem 72810
  asset end
  numberOfLiquidations 12
  liquidation start 0
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 25467159236203619793758
  collateralVTokenGained 312894180801642
  collateralUnderlyingGained 63284686050
  gasUsed 1369946
  postHealthFactor 0.888734247444888248
  repaidUsd 25486.863686647855258464
  seizedUsd 28035.550053096303000000
  liquidation end
  liquidation start 1
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 12733579618101809896879
  collateralVTokenGained 156447090400821
  collateralUnderlyingGained 31642343025
  gasUsed 490744
  postHealthFactor 0.888829616729877829
  repaidUsd 12743.431843323927629232
  seizedUsd 14017.775026548151500000
  liquidation end
  liquidation start 2
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 6366789809050945644561
  collateralVTokenGained 78223545200411
  collateralUnderlyingGained 15821171512
  gasUsed 490755
  postHealthFactor 0.888878086662883938
  repaidUsd 6371.715921662004542225
  seizedUsd 7008.887513052572320000
  liquidation end
  liquidation start 3
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 3183394904525432126159
  collateralVTokenGained 39111772600205
  collateralUnderlyingGained 7910585756
  gasUsed 490765
  postHealthFactor 0.888902521734372799
  repaidUsd 3185.857960830961543503
  seizedUsd 3504.443756526286160000
  liquidation end
  liquidation start 4
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 1591697452262756759202
  collateralVTokenGained 19555886300103
  collateralUnderlyingGained 3955292878
  gasUsed 490777
  postHealthFactor 0.888914789778931390
  repaidUsd 1592.928980415521499361
  seizedUsd 1752.221878263143080000
  liquidation end
  liquidation start 5
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 795848726131337683479
  collateralVTokenGained 9777943150051
  collateralUnderlyingGained 1977646439
  gasUsed 490787
  postHealthFactor 0.888920936489359054
  repaidUsd 796.464490207720022071
  seizedUsd 876.110939131571540000
  liquidation end
  liquidation start 6
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 397924363065709537862
  collateralVTokenGained 4888971575026
  collateralUnderlyingGained 988823219
  gasUsed 490798
  postHealthFactor 0.888924013024267492
  repaidUsd 398.232245103900738645
  seizedUsd 438.055469344282340000
  liquidation end
  liquidation start 7
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 198962181532854768931
  collateralVTokenGained 2444485787513
  collateralUnderlyingGained 494411609
  gasUsed 490809
  postHealthFactor 0.888925552087605026
  repaidUsd 199.116122551950369322
  seizedUsd 219.027734450637740000
  liquidation end
  liquidation start 8
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 99481090766386688344
  collateralVTokenGained 1222242893756
  collateralUnderlyingGained 247205804
  gasUsed 490819
  postHealthFactor 0.888926321818364736
  repaidUsd 99.558061275934457052
  seizedUsd 109.513867003815440000
  liquidation end
  liquidation start 9
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 49740545383193344172
  collateralVTokenGained 611121446878
  collateralUnderlyingGained 123602902
  gasUsed 490830
  postHealthFactor 0.888926706733532350
  repaidUsd 49.779030637967228526
  seizedUsd 54.756933501907720000
  liquidation end
  liquidation start 10
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 24870272691596672086
  collateralVTokenGained 305560723439
  collateralUnderlyingGained 61801451
  gasUsed 490841
  postHealthFactor 0.888926899203564976
  repaidUsd 24.889515318983614263
  seizedUsd 27.378466750953860000
  liquidation end
  liquidation start 11
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 12435136345839032165
  collateralVTokenGained 152780361720
  collateralUnderlyingGained 30900725
  gasUsed 490851
  postHealthFactor 0.888926995441693728
  repaidUsd 12.444757659532534740
  seizedUsd 13.689233153973500000
  liquidation end
  gasUsage start
  approves 25219
  liquidations 6768722
  redeems 72810
  total 6866751
  gasUsage end
  gasPrice 5000000000
  chainCoinPrice 307500000000000000000
  gasFeeUsd 10.557629662500000000
  repaidUsd 50961.282615636259437410
  seizedUsd 56057.410873481639360000
  profitUsd 5085.570628182879922590
  strategyRunReport end
  Tests case end
  
  Tests case: largestBorrow
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 115372.552942183329304308 126909.808228869839640000 2.103426075000000000
  
  strategyRunReport start
  initialHealthFactor 0.888549544472118463
  finalHealthFactor 0.889454541592519728
  asset start 0
  initialData start
  symbol vETH
  vtoken 0xf508fCD89b8bd15579dc79A6827cB4686A3592c8
  collateralFactor 800000000000000000
  collateralAmount 1
  borrowAmount 0
  cash 98733251973295417280333
  exchangeRate 202256200323554818629002859
  price 2417380000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000000488930092756
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 1
  initialData start
  symbol vBNB
  vtoken 0xA07c5b74C9B40447a954e1466938b865b6BBea36
  collateralFactor 800000000000000000
  collateralAmount 90679
  borrowAmount 6738714
  cash 451409168257442668724203
  exchangeRate 217034279468836014278688526
  price 307500000000000000000
  borrowValueUsd 0.000000002072154555
  collateralValueUsd 0.006051738814095855
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 2
  initialData start
  symbol vLUNA
  vtoken 0xb91A659E88B51474767CD97EF3196A3e7cEDD2c8
  collateralFactor 550000000000000000
  collateralAmount 4694330665
  borrowAmount 0
  cash 2299686119
  exchangeRate 201572303931386
  price 1683788410000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 1.593279731597270000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 3
  initialData start
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 4
  initialData start
  symbol vUSDC
  vtoken 0xecA88125a5ADbe82614ffC12D0DB554E2e2867C8
  collateralFactor 800000000000000000
  collateralAmount 358766824
  borrowAmount 567988636611363632257261
  cash 40995423278631310392585040
  exchangeRate 215004843382592409562485380
  price 1000460330000000000
  borrowValueUsd 568250.098820454941278097
  collateralValueUsd 0.077172113098279976
  isCollateralCappedByCash false
  initialData end
  repaid 115319467931510417113998
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 1
  gasUsedToApprove 26093
  gasUsedToRedeem 0
  asset end
  asset start 5
  initialData start
  symbol vUSDT
  vtoken 0xfD5840Cd36d94D7229439859C0112a4185BC0255
  collateralFactor 800000000000000000
  collateralAmount 2260569030
  borrowAmount 18456180601049713191612
  cash 167249425209188013677527166
  exchangeRate 217035732487126732238426137
  price 1000000000000000000
  borrowValueUsd 18456.180601049713191612
  collateralValueUsd 0.490624255263763564
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 6
  initialData start
  symbol vCAKE
  vtoken 0x86aC3974e2BD0d60825230fa6F355fF11409df5c
  collateralFactor 550000000000000000
  collateralAmount 965
  borrowAmount 0
  cash 934730615905852945604718
  exchangeRate 246614296373284591063478232
  price 5250000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000001249409679000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 7
  initialData start
  symbol vTUSD
  vtoken 0x08CEB3F4a7ed3500cA0982bcd0FC7816688084c3
  collateralFactor 800000000000000000
  collateralAmount 0
  borrowAmount 567637659718080460924470
  cash 170322575017834871677866389
  exchangeRate 204113187854884466431952156
  price 1000000000000000000
  borrowValueUsd 567637.659718080460924470
  collateralValueUsd 0.000000000000000000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 8
  initialData start
  symbol vDOT
  vtoken 0x1610bc33319e9398de5f57B33a5b184c806aD217
  collateralFactor 600000000000000000
  collateralAmount 13293051
  borrowAmount 0
  cash 1290978535920025293841317
  exchangeRate 211386371467589062910113234
  price 10670000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.029982377943373876
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 9
  initialData start
  symbol vADA
  vtoken 0x9A0AF7FDb2065Ce470D72664DE73cAE409dA28Ec
  collateralFactor 600000000000000000
  collateralAmount 841125259
  borrowAmount 0
  cash 17527718238816535389170835
  exchangeRate 201470313645614750255556850
  price 631365700000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.106992348878908816
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 10
  initialData start
  symbol vMATIC
  vtoken 0x5c9476FcD6a4F9a3654139721c949c2233bBbBc8
  collateralFactor 600000000000000000
  collateralAmount 2733010282
  borrowAmount 0
  cash 2534808075149721012286791
  exchangeRate 203378028283301843803274252
  price 841068440000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.467494639180149767
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 11
  initialData start
  symbol vAAVE
  vtoken 0x26DA28954763B92139ED49283625ceCAf52C6f94
  collateralFactor 550000000000000000
  collateralAmount 36403409
  borrowAmount 0
  cash 12197275182207762794244
  exchangeRate 202601019036947906273617883
  price 102780552200000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.758044371032253235
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 12
  initialData start
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  initialData end
  repaid 0
  collateralVTokenGained 1416392416265723
  collateralUnderlyingGained 286473686274
  liquidationsParticipated 1
  gasUsedToApprove 0
  gasUsedToRedeem 72832
  asset end
  numberOfLiquidations 1
  liquidation start 0
  repaySymbol vUSDC
  collateralSymbol vUST
  repayVToken 0xecA88125a5ADbe82614ffC12D0DB554E2e2867C8
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 115319467931510417113998
  collateralVTokenGained 1416392416265723
  collateralUnderlyingGained 286473686274
  gasUsed 1368082
  postHealthFactor 0.889454541592519728
  repaidUsd 115372.552942183329304308
  seizedUsd 126909.808228869839640000
  liquidation end
  gasUsage start
  approves 26093
  liquidations 1368082
  redeems 72832
  total 1467007
  gasUsage end
  gasPrice 5000000000
  chainCoinPrice 307500000000000000000
  gasFeeUsd 2.255523262500000000
  repaidUsd 115372.552942183329304308
  seizedUsd 126909.808228869839640000
  profitUsd 11534.999763424010335692
  strategyRunReport end
  Tests case end
  
  Tests case: drainSameToken
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.000557310557793760 0.000613041613531098 2.008872900000000000
  liquidation does not cover gas cost (revenue, cost) 0.000055731055737338 2.008872900000000000
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.070156466454336051 0.077172113098279976 1.998287212500000000
  liquidation does not cover gas cost (revenue, cost) 0.007015646643943925 1.998287212500000000
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.446022050306775974 0.490624255263763564 1.977375675000000000
  liquidation does not cover gas cost (revenue, cost) 0.044602204956987590 1.977375675000000000
  
  strategyRunReport start
  initialHealthFactor 0.888549544472118463
  finalHealthFactor 0.888549544472118463
  asset start 0
  initialData start
  symbol vETH
  vtoken 0xf508fCD89b8bd15579dc79A6827cB4686A3592c8
  collateralFactor 800000000000000000
  collateralAmount 1
  borrowAmount 0
  cash 98733251973295417280333
  exchangeRate 202256200323554818629002859
  price 2417380000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000000488930092756
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 1
  initialData start
  symbol vBNB
  vtoken 0xA07c5b74C9B40447a954e1466938b865b6BBea36
  collateralFactor 800000000000000000
  collateralAmount 90679
  borrowAmount 6738714
  cash 451409168257442668724203
  exchangeRate 217034279468836014278688526
  price 307500000000000000000
  borrowValueUsd 0.000000002072154555
  collateralValueUsd 0.006051738814095855
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 2
  initialData start
  symbol vLUNA
  vtoken 0xb91A659E88B51474767CD97EF3196A3e7cEDD2c8
  collateralFactor 550000000000000000
  collateralAmount 4694330665
  borrowAmount 0
  cash 2299686119
  exchangeRate 201572303931386
  price 1683788410000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 1.593279731597270000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 3
  initialData start
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 25408
  gasUsedToRedeem 0
  asset end
  asset start 4
  initialData start
  symbol vUSDC
  vtoken 0xecA88125a5ADbe82614ffC12D0DB554E2e2867C8
  collateralFactor 800000000000000000
  collateralAmount 358766824
  borrowAmount 567988636611363632257261
  cash 40995423278631310392585040
  exchangeRate 215004843382592409562485380
  price 1000460330000000000
  borrowValueUsd 568250.098820454941278097
  collateralValueUsd 0.077172113098279976
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 26158
  gasUsedToRedeem 0
  asset end
  asset start 5
  initialData start
  symbol vUSDT
  vtoken 0xfD5840Cd36d94D7229439859C0112a4185BC0255
  collateralFactor 800000000000000000
  collateralAmount 2260569030
  borrowAmount 18456180601049713191612
  cash 167249425209188013677527166
  exchangeRate 217035732487126732238426137
  price 1000000000000000000
  borrowValueUsd 18456.180601049713191612
  collateralValueUsd 0.490624255263763564
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 25416
  gasUsedToRedeem 0
  asset end
  asset start 6
  initialData start
  symbol vCAKE
  vtoken 0x86aC3974e2BD0d60825230fa6F355fF11409df5c
  collateralFactor 550000000000000000
  collateralAmount 965
  borrowAmount 0
  cash 934730615905852945604718
  exchangeRate 246614296373284591063478232
  price 5250000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000001249409679000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 7
  initialData start
  symbol vTUSD
  vtoken 0x08CEB3F4a7ed3500cA0982bcd0FC7816688084c3
  collateralFactor 800000000000000000
  collateralAmount 0
  borrowAmount 567637659718080460924470
  cash 170322575017834871677866389
  exchangeRate 204113187854884466431952156
  price 1000000000000000000
  borrowValueUsd 567637.659718080460924470
  collateralValueUsd 0.000000000000000000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 8
  initialData start
  symbol vDOT
  vtoken 0x1610bc33319e9398de5f57B33a5b184c806aD217
  collateralFactor 600000000000000000
  collateralAmount 13293051
  borrowAmount 0
  cash 1290978535920025293841317
  exchangeRate 211386371467589062910113234
  price 10670000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.029982377943373876
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 9
  initialData start
  symbol vADA
  vtoken 0x9A0AF7FDb2065Ce470D72664DE73cAE409dA28Ec
  collateralFactor 600000000000000000
  collateralAmount 841125259
  borrowAmount 0
  cash 17527718238816535389170835
  exchangeRate 201470313645614750255556850
  price 631365700000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.106992348878908816
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 10
  initialData start
  symbol vMATIC
  vtoken 0x5c9476FcD6a4F9a3654139721c949c2233bBbBc8
  collateralFactor 600000000000000000
  collateralAmount 2733010282
  borrowAmount 0
  cash 2534808075149721012286791
  exchangeRate 203378028283301843803274252
  price 841068440000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.467494639180149767
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 11
  initialData start
  symbol vAAVE
  vtoken 0x26DA28954763B92139ED49283625ceCAf52C6f94
  collateralFactor 550000000000000000
  collateralAmount 36403409
  borrowAmount 0
  cash 12197275182207762794244
  exchangeRate 202601019036947906273617883
  price 102780552200000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.758044371032253235
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 12
  initialData start
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  numberOfLiquidations 0
  gasUsage start
  approves 76982
  liquidations 0
  redeems 0
  total 76982
  gasUsage end
  gasPrice 5000000000
  chainCoinPrice 307500000000000000000
  gasFeeUsd 0.118359825000000000
  repaidUsd 0.000000000000000000
  seizedUsd 0.000000000000000000
  profitUsd -0.118359825000000000
  strategyRunReport end
  Tests case end
  
  Tests case: largestCollateralFactorFirst
  repayAsset:
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  collateralAssets:
  asset start 0
  symbol vETH
  vtoken 0xf508fCD89b8bd15579dc79A6827cB4686A3592c8
  collateralFactor 800000000000000000
  collateralAmount 1
  borrowAmount 0
  cash 98733251973295417280333
  exchangeRate 202256200323554818629002859
  price 2417380000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000000488930092756
  isCollateralCappedByCash false
  asset end
  asset start 1
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  asset end
  asset start 2
  symbol vBNB
  vtoken 0xA07c5b74C9B40447a954e1466938b865b6BBea36
  collateralFactor 800000000000000000
  collateralAmount 90679
  borrowAmount 6738714
  cash 451409168257442668724203
  exchangeRate 217034279468836014278688526
  price 307500000000000000000
  borrowValueUsd 0.000000002072154555
  collateralValueUsd 0.006051738814095855
  isCollateralCappedByCash false
  asset end
  asset start 3
  symbol vUSDC
  vtoken 0xecA88125a5ADbe82614ffC12D0DB554E2e2867C8
  collateralFactor 800000000000000000
  collateralAmount 358766824
  borrowAmount 567988636611363632257261
  cash 40995423278631310392585040
  exchangeRate 215004843382592409562485380
  price 1000460330000000000
  borrowValueUsd 568250.098820454941278097
  collateralValueUsd 0.077172113098279976
  isCollateralCappedByCash false
  asset end
  asset start 4
  symbol vUSDT
  vtoken 0xfD5840Cd36d94D7229439859C0112a4185BC0255
  collateralFactor 800000000000000000
  collateralAmount 2260569030
  borrowAmount 18456180601049713191612
  cash 167249425209188013677527166
  exchangeRate 217035732487126732238426137
  price 1000000000000000000
  borrowValueUsd 18456.180601049713191612
  collateralValueUsd 0.490624255263763564
  isCollateralCappedByCash false
  asset end
  asset start 5
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  asset end
  asset start 6
  symbol vDOT
  vtoken 0x1610bc33319e9398de5f57B33a5b184c806aD217
  collateralFactor 600000000000000000
  collateralAmount 13293051
  borrowAmount 0
  cash 1290978535920025293841317
  exchangeRate 211386371467589062910113234
  price 10670000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.029982377943373876
  isCollateralCappedByCash false
  asset end
  asset start 7
  symbol vADA
  vtoken 0x9A0AF7FDb2065Ce470D72664DE73cAE409dA28Ec
  collateralFactor 600000000000000000
  collateralAmount 841125259
  borrowAmount 0
  cash 17527718238816535389170835
  exchangeRate 201470313645614750255556850
  price 631365700000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.106992348878908816
  isCollateralCappedByCash false
  asset end
  asset start 8
  symbol vMATIC
  vtoken 0x5c9476FcD6a4F9a3654139721c949c2233bBbBc8
  collateralFactor 600000000000000000
  collateralAmount 2733010282
  borrowAmount 0
  cash 2534808075149721012286791
  exchangeRate 203378028283301843803274252
  price 841068440000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.467494639180149767
  isCollateralCappedByCash false
  asset end
  asset start 9
  symbol vCAKE
  vtoken 0x86aC3974e2BD0d60825230fa6F355fF11409df5c
  collateralFactor 550000000000000000
  collateralAmount 965
  borrowAmount 0
  cash 934730615905852945604718
  exchangeRate 246614296373284591063478232
  price 5250000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000001249409679000
  isCollateralCappedByCash false
  asset end
  asset start 10
  symbol vAAVE
  vtoken 0x26DA28954763B92139ED49283625ceCAf52C6f94
  collateralFactor 550000000000000000
  collateralAmount 36403409
  borrowAmount 0
  cash 12197275182207762794244
  exchangeRate 202601019036947906273617883
  price 102780552200000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.758044371032253235
  isCollateralCappedByCash false
  asset end
  asset start 11
  symbol vLUNA
  vtoken 0xb91A659E88B51474767CD97EF3196A3e7cEDD2c8
  collateralFactor 550000000000000000
  collateralAmount 4694330665
  borrowAmount 0
  cash 2299686119
  exchangeRate 201572303931386
  price 1683788410000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 1.593279731597270000
  isCollateralCappedByCash false
  asset end
  
  cycleIndex 0
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 12
  currentCollateral vETH
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.000000444481923793 0.000000488930092756 2.083051125000000000
  liquidation does not cover gas cost (revenue, cost) 0.000000044448168963 2.083051125000000000
  
  cycleIndex 1
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 11
  currentCollateral vBUSD
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.000557310557793760 0.000613041613531098 2.009111212500000000
  liquidation does not cover gas cost (revenue, cost) 0.000055731055737338 2.009111212500000000
  
  cycleIndex 2
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 10
  currentCollateral vBNB
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.005501580902594988 0.006051738814095855 2.054259900000000000
  liquidation does not cover gas cost (revenue, cost) 0.000550157911500867 2.054259900000000000
  
  cycleIndex 3
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 9
  currentCollateral vUSDC
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.070156466465929034 0.077172113098279976 2.071919625000000000
  liquidation does not cover gas cost (revenue, cost) 0.007015646632350942 2.071919625000000000
  
  cycleIndex 4
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 8
  currentCollateral vUSDT
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.446022050326294629 0.490624255263763564 2.067460875000000000
  liquidation does not cover gas cost (revenue, cost) 0.044602204937468935 2.067460875000000000
  
  cycleIndex 5
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 7
  currentCollateral vUST
  other collaterals are smaller, trying drain strategy
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 25486.863686647855258464 28035.550053096303000000 2.107128375000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 12743.431843323927629232 14017.775026548151500000 0.755358375000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6371.715921662004542225 7008.887513052572320000 0.755375287500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 3185.857960830961543503 3504.443756526286160000 0.755390662500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 1592.928980415521499361 1752.221878263143080000 0.755406037500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 796.464490207720022071 876.110939131571540000 0.755424487500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 398.232245103900738645 438.055469344282340000 0.755439862500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 199.116122551950369322 219.027734450637740000 0.755456775000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 99.558061275934457052 109.513867003815440000 0.755472150000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 49.779030637967228526 54.756933501907720000 0.755489062500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 24.889515318983614263 27.378466750953860000 0.755505975000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 12.444757659532534740 13.689233153973500000 0.755521350000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6.222378829766267370 6.844616355483320000 0.755538262500000000
  liquidation does not cover gas cost (revenue, cost) 0.622237525717052630 0.755538262500000000
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6.222378829766267370 6.844616355483320000 0.755545950000000000
  liquidation does not cover gas cost (revenue, cost) 0.622237525717052630 0.755545950000000000
  
  strategyRunReport start
  initialHealthFactor 0.888549544472118463
  finalHealthFactor 0.888926995441693728
  asset start 0
  initialData start
  symbol vETH
  vtoken 0xf508fCD89b8bd15579dc79A6827cB4686A3592c8
  collateralFactor 800000000000000000
  collateralAmount 1
  borrowAmount 0
  cash 98733251973295417280333
  exchangeRate 202256200323554818629002859
  price 2417380000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000000488930092756
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 1
  initialData start
  symbol vBNB
  vtoken 0xA07c5b74C9B40447a954e1466938b865b6BBea36
  collateralFactor 800000000000000000
  collateralAmount 90679
  borrowAmount 6738714
  cash 451409168257442668724203
  exchangeRate 217034279468836014278688526
  price 307500000000000000000
  borrowValueUsd 0.000000002072154555
  collateralValueUsd 0.006051738814095855
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 2
  initialData start
  symbol vLUNA
  vtoken 0xb91A659E88B51474767CD97EF3196A3e7cEDD2c8
  collateralFactor 550000000000000000
  collateralAmount 4694330665
  borrowAmount 0
  cash 2299686119
  exchangeRate 201572303931386
  price 1683788410000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 1.593279731597270000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 3
  initialData start
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  initialData end
  repaid 50921883336061481947598
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 12
  gasUsedToApprove 25476
  gasUsedToRedeem 0
  asset end
  asset start 4
  initialData start
  symbol vUSDC
  vtoken 0xecA88125a5ADbe82614ffC12D0DB554E2e2867C8
  collateralFactor 800000000000000000
  collateralAmount 358766824
  borrowAmount 567988636611363632257261
  cash 40995423278631310392585040
  exchangeRate 215004843382592409562485380
  price 1000460330000000000
  borrowValueUsd 568250.098820454941278097
  collateralValueUsd 0.077172113098279976
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 5
  initialData start
  symbol vUSDT
  vtoken 0xfD5840Cd36d94D7229439859C0112a4185BC0255
  collateralFactor 800000000000000000
  collateralAmount 2260569030
  borrowAmount 18456180601049713191612
  cash 167249425209188013677527166
  exchangeRate 217035732487126732238426137
  price 1000000000000000000
  borrowValueUsd 18456.180601049713191612
  collateralValueUsd 0.490624255263763564
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 6
  initialData start
  symbol vCAKE
  vtoken 0x86aC3974e2BD0d60825230fa6F355fF11409df5c
  collateralFactor 550000000000000000
  collateralAmount 965
  borrowAmount 0
  cash 934730615905852945604718
  exchangeRate 246614296373284591063478232
  price 5250000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000001249409679000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 7
  initialData start
  symbol vTUSD
  vtoken 0x08CEB3F4a7ed3500cA0982bcd0FC7816688084c3
  collateralFactor 800000000000000000
  collateralAmount 0
  borrowAmount 567637659718080460924470
  cash 170322575017834871677866389
  exchangeRate 204113187854884466431952156
  price 1000000000000000000
  borrowValueUsd 567637.659718080460924470
  collateralValueUsd 0.000000000000000000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 8
  initialData start
  symbol vDOT
  vtoken 0x1610bc33319e9398de5f57B33a5b184c806aD217
  collateralFactor 600000000000000000
  collateralAmount 13293051
  borrowAmount 0
  cash 1290978535920025293841317
  exchangeRate 211386371467589062910113234
  price 10670000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.029982377943373876
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 9
  initialData start
  symbol vADA
  vtoken 0x9A0AF7FDb2065Ce470D72664DE73cAE409dA28Ec
  collateralFactor 600000000000000000
  collateralAmount 841125259
  borrowAmount 0
  cash 17527718238816535389170835
  exchangeRate 201470313645614750255556850
  price 631365700000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.106992348878908816
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 10
  initialData start
  symbol vMATIC
  vtoken 0x5c9476FcD6a4F9a3654139721c949c2233bBbBc8
  collateralFactor 600000000000000000
  collateralAmount 2733010282
  borrowAmount 0
  cash 2534808075149721012286791
  exchangeRate 203378028283301843803274252
  price 841068440000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.467494639180149767
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 11
  initialData start
  symbol vAAVE
  vtoken 0x26DA28954763B92139ED49283625ceCAf52C6f94
  collateralFactor 550000000000000000
  collateralAmount 36403409
  borrowAmount 0
  cash 12197275182207762794244
  exchangeRate 202601019036947906273617883
  price 102780552200000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.758044371032253235
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 12
  initialData start
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  initialData end
  repaid 0
  collateralVTokenGained 625635581241565
  collateralUnderlyingGained 126538471376
  liquidationsParticipated 12
  gasUsedToApprove 0
  gasUsedToRedeem 72992
  asset end
  numberOfLiquidations 12
  liquidation start 0
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 25467159236203619793758
  collateralVTokenGained 312894180801642
  collateralUnderlyingGained 63284686050
  gasUsed 1370490
  postHealthFactor 0.888734247444888248
  repaidUsd 25486.863686647855258464
  seizedUsd 28035.550053096303000000
  liquidation end
  liquidation start 1
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 12733579618101809896879
  collateralVTokenGained 156447090400821
  collateralUnderlyingGained 31642343025
  gasUsed 491290
  postHealthFactor 0.888829616729877829
  repaidUsd 12743.431843323927629232
  seizedUsd 14017.775026548151500000
  liquidation end
  liquidation start 2
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 6366789809050945644561
  collateralVTokenGained 78223545200411
  collateralUnderlyingGained 15821171512
  gasUsed 491301
  postHealthFactor 0.888878086662883938
  repaidUsd 6371.715921662004542225
  seizedUsd 7008.887513052572320000
  liquidation end
  liquidation start 3
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 3183394904525432126159
  collateralVTokenGained 39111772600205
  collateralUnderlyingGained 7910585756
  gasUsed 491311
  postHealthFactor 0.888902521734372799
  repaidUsd 3185.857960830961543503
  seizedUsd 3504.443756526286160000
  liquidation end
  liquidation start 4
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 1591697452262756759202
  collateralVTokenGained 19555886300103
  collateralUnderlyingGained 3955292878
  gasUsed 491321
  postHealthFactor 0.888914789778931390
  repaidUsd 1592.928980415521499361
  seizedUsd 1752.221878263143080000
  liquidation end
  liquidation start 5
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 795848726131337683479
  collateralVTokenGained 9777943150051
  collateralUnderlyingGained 1977646439
  gasUsed 491333
  postHealthFactor 0.888920936489359054
  repaidUsd 796.464490207720022071
  seizedUsd 876.110939131571540000
  liquidation end
  liquidation start 6
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 397924363065709537862
  collateralVTokenGained 4888971575026
  collateralUnderlyingGained 988823219
  gasUsed 491343
  postHealthFactor 0.888924013024267492
  repaidUsd 398.232245103900738645
  seizedUsd 438.055469344282340000
  liquidation end
  liquidation start 7
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 198962181532854768931
  collateralVTokenGained 2444485787513
  collateralUnderlyingGained 494411609
  gasUsed 491354
  postHealthFactor 0.888925552087605026
  repaidUsd 199.116122551950369322
  seizedUsd 219.027734450637740000
  liquidation end
  liquidation start 8
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 99481090766386688344
  collateralVTokenGained 1222242893756
  collateralUnderlyingGained 247205804
  gasUsed 491364
  postHealthFactor 0.888926321818364736
  repaidUsd 99.558061275934457052
  seizedUsd 109.513867003815440000
  liquidation end
  liquidation start 9
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 49740545383193344172
  collateralVTokenGained 611121446878
  collateralUnderlyingGained 123602902
  gasUsed 491375
  postHealthFactor 0.888926706733532350
  repaidUsd 49.779030637967228526
  seizedUsd 54.756933501907720000
  liquidation end
  liquidation start 10
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 24870272691596672086
  collateralVTokenGained 305560723439
  collateralUnderlyingGained 61801451
  gasUsed 491386
  postHealthFactor 0.888926899203564976
  repaidUsd 24.889515318983614263
  seizedUsd 27.378466750953860000
  liquidation end
  liquidation start 11
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 12435136345839032165
  collateralVTokenGained 152780361720
  collateralUnderlyingGained 30900725
  gasUsed 491396
  postHealthFactor 0.888926995441693728
  repaidUsd 12.444757659532534740
  seizedUsd 13.689233153973500000
  liquidation end
  gasUsage start
  approves 25476
  liquidations 6775264
  redeems 72992
  total 6873732
  gasUsage end
  gasPrice 5000000000
  chainCoinPrice 307500000000000000000
  gasFeeUsd 10.568362950000000000
  repaidUsd 50961.282615636259437410
  seizedUsd 56057.410873481639360000
  profitUsd 5085.559894895379922590
  strategyRunReport end
  Tests case end
  
  Tests case: smallestCollateralFactorFirst
  repayAsset:
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  collateralAssets:
  asset start 0
  symbol vCAKE
  vtoken 0x86aC3974e2BD0d60825230fa6F355fF11409df5c
  collateralFactor 550000000000000000
  collateralAmount 965
  borrowAmount 0
  cash 934730615905852945604718
  exchangeRate 246614296373284591063478232
  price 5250000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000001249409679000
  isCollateralCappedByCash false
  asset end
  asset start 1
  symbol vAAVE
  vtoken 0x26DA28954763B92139ED49283625ceCAf52C6f94
  collateralFactor 550000000000000000
  collateralAmount 36403409
  borrowAmount 0
  cash 12197275182207762794244
  exchangeRate 202601019036947906273617883
  price 102780552200000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.758044371032253235
  isCollateralCappedByCash false
  asset end
  asset start 2
  symbol vLUNA
  vtoken 0xb91A659E88B51474767CD97EF3196A3e7cEDD2c8
  collateralFactor 550000000000000000
  collateralAmount 4694330665
  borrowAmount 0
  cash 2299686119
  exchangeRate 201572303931386
  price 1683788410000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 1.593279731597270000
  isCollateralCappedByCash false
  asset end
  asset start 3
  symbol vDOT
  vtoken 0x1610bc33319e9398de5f57B33a5b184c806aD217
  collateralFactor 600000000000000000
  collateralAmount 13293051
  borrowAmount 0
  cash 1290978535920025293841317
  exchangeRate 211386371467589062910113234
  price 10670000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.029982377943373876
  isCollateralCappedByCash false
  asset end
  asset start 4
  symbol vADA
  vtoken 0x9A0AF7FDb2065Ce470D72664DE73cAE409dA28Ec
  collateralFactor 600000000000000000
  collateralAmount 841125259
  borrowAmount 0
  cash 17527718238816535389170835
  exchangeRate 201470313645614750255556850
  price 631365700000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.106992348878908816
  isCollateralCappedByCash false
  asset end
  asset start 5
  symbol vMATIC
  vtoken 0x5c9476FcD6a4F9a3654139721c949c2233bBbBc8
  collateralFactor 600000000000000000
  collateralAmount 2733010282
  borrowAmount 0
  cash 2534808075149721012286791
  exchangeRate 203378028283301843803274252
  price 841068440000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.467494639180149767
  isCollateralCappedByCash false
  asset end
  asset start 6
  symbol vETH
  vtoken 0xf508fCD89b8bd15579dc79A6827cB4686A3592c8
  collateralFactor 800000000000000000
  collateralAmount 1
  borrowAmount 0
  cash 98733251973295417280333
  exchangeRate 202256200323554818629002859
  price 2417380000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000000488930092756
  isCollateralCappedByCash false
  asset end
  asset start 7
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  asset end
  asset start 8
  symbol vBNB
  vtoken 0xA07c5b74C9B40447a954e1466938b865b6BBea36
  collateralFactor 800000000000000000
  collateralAmount 90679
  borrowAmount 6738714
  cash 451409168257442668724203
  exchangeRate 217034279468836014278688526
  price 307500000000000000000
  borrowValueUsd 0.000000002072154555
  collateralValueUsd 0.006051738814095855
  isCollateralCappedByCash false
  asset end
  asset start 9
  symbol vUSDC
  vtoken 0xecA88125a5ADbe82614ffC12D0DB554E2e2867C8
  collateralFactor 800000000000000000
  collateralAmount 358766824
  borrowAmount 567988636611363632257261
  cash 40995423278631310392585040
  exchangeRate 215004843382592409562485380
  price 1000460330000000000
  borrowValueUsd 568250.098820454941278097
  collateralValueUsd 0.077172113098279976
  isCollateralCappedByCash false
  asset end
  asset start 10
  symbol vUSDT
  vtoken 0xfD5840Cd36d94D7229439859C0112a4185BC0255
  collateralFactor 800000000000000000
  collateralAmount 2260569030
  borrowAmount 18456180601049713191612
  cash 167249425209188013677527166
  exchangeRate 217035732487126732238426137
  price 1000000000000000000
  borrowValueUsd 18456.180601049713191612
  collateralValueUsd 0.490624255263763564
  isCollateralCappedByCash false
  asset end
  asset start 11
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  asset end
  
  cycleIndex 0
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 12
  currentCollateral vCAKE
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.000001135826981473 0.000001249409679000 2.091453562500000000
  liquidation does not cover gas cost (revenue, cost) 0.000000113582697527 2.091453562500000000
  
  cycleIndex 1
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 11
  currentCollateral vAAVE
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.689131255397240700 0.758044371032253235 2.096033775000000000
  liquidation does not cover gas cost (revenue, cost) 0.068913115635012535 2.096033775000000000
  
  cycleIndex 2
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 10
  currentCollateral vLUNA
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 1.448436192848697200 1.593279731597270000 2.107632675000000000
  liquidation does not cover gas cost (revenue, cost) 0.144843538748572800 2.107632675000000000
  
  cycleIndex 3
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 9
  currentCollateral vDOT
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.027256707271051645 0.029982377943373876 2.083769137500000000
  liquidation does not cover gas cost (revenue, cost) 0.002725670672322231 2.083769137500000000
  
  cycleIndex 4
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 8
  currentCollateral vADA
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.097265771713162704 0.106992348878908816 2.083812187500000000
  liquidation does not cover gas cost (revenue, cost) 0.009726577165746112 2.083812187500000000
  
  cycleIndex 5
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 7
  currentCollateral vMATIC
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.424995126552485137 0.467494639180149767 2.096205975000000000
  liquidation does not cover gas cost (revenue, cost) 0.042499512627664630 2.096205975000000000
  
  cycleIndex 6
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 6
  currentCollateral vETH
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.000000444481923793 0.000000488930092756 2.083925962500000000
  liquidation does not cover gas cost (revenue, cost) 0.000000044448168963 2.083925962500000000
  
  cycleIndex 7
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 5
  currentCollateral vBUSD
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.000557310557793760 0.000613041613531098 2.009981437500000000
  liquidation does not cover gas cost (revenue, cost) 0.000055731055737338 2.009981437500000000
  
  cycleIndex 8
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 4
  currentCollateral vBNB
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.005501580902594988 0.006051738814095855 2.055127050000000000
  liquidation does not cover gas cost (revenue, cost) 0.000550157911500867 2.055127050000000000
  
  cycleIndex 9
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 3
  currentCollateral vUSDC
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.070156466465929034 0.077172113098279976 2.072783700000000000
  liquidation does not cover gas cost (revenue, cost) 0.007015646632350942 2.072783700000000000
  
  cycleIndex 10
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 2
  currentCollateral vUSDT
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 0.446022050326294629 0.490624255263763564 2.068318800000000000
  liquidation does not cover gas cost (revenue, cost) 0.044602204937468935 2.068318800000000000
  
  cycleIndex 11
  healthFactor 0.888549544298614287
  numberOfCollateralAssets 1
  currentCollateral vUST
  only one collateral left, doing drain
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 25486.863686647855258464 28035.550053096303000000 2.107983225000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 12743.431843323927629232 14017.775026548151500000 0.756211687500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6371.715921662004542225 7008.887513052572320000 0.756228600000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 3185.857960830961543503 3504.443756526286160000 0.756245512500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 1592.928980415521499361 1752.221878263143080000 0.756260887500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 796.464490207720022071 876.110939131571540000 0.756277800000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 398.232245103900738645 438.055469344282340000 0.756293175000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 199.116122551950369322 219.027734450637740000 0.756310087500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 99.558061275934457052 109.513867003815440000 0.756325462500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 49.779030637967228526 54.756933501907720000 0.756343912500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 24.889515318983614263 27.378466750953860000 0.756359287500000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 12.444757659532534740 13.689233153973500000 0.756376200000000000
  Max repayAmount will keep the borrower liquidatable!!!
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6.222378829766267370 6.844616355483320000 0.756393112500000000
  liquidation does not cover gas cost (revenue, cost) 0.622237525717052630 0.756393112500000000
  did a liquidation (repayUsd, seizedUsd, gasInUsd) 6.222378829766267370 6.844616355483320000 0.756399262500000000
  liquidation does not cover gas cost (revenue, cost) 0.622237525717052630 0.756399262500000000
  
  strategyRunReport start
  initialHealthFactor 0.888549544472118463
  finalHealthFactor 0.888926995441693728
  asset start 0
  initialData start
  symbol vETH
  vtoken 0xf508fCD89b8bd15579dc79A6827cB4686A3592c8
  collateralFactor 800000000000000000
  collateralAmount 1
  borrowAmount 0
  cash 98733251973295417280333
  exchangeRate 202256200323554818629002859
  price 2417380000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000000488930092756
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 1
  initialData start
  symbol vBNB
  vtoken 0xA07c5b74C9B40447a954e1466938b865b6BBea36
  collateralFactor 800000000000000000
  collateralAmount 90679
  borrowAmount 6738714
  cash 451409168257442668724203
  exchangeRate 217034279468836014278688526
  price 307500000000000000000
  borrowValueUsd 0.000000002072154555
  collateralValueUsd 0.006051738814095855
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 2
  initialData start
  symbol vLUNA
  vtoken 0xb91A659E88B51474767CD97EF3196A3e7cEDD2c8
  collateralFactor 550000000000000000
  collateralAmount 4694330665
  borrowAmount 0
  cash 2299686119
  exchangeRate 201572303931386
  price 1683788410000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 1.593279731597270000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 3
  initialData start
  symbol vBUSD
  vtoken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralFactor 800000000000000000
  collateralAmount 2843494
  borrowAmount 50934318472407332277607
  cash 100004410132855411882486316
  exchangeRate 215427800334024428630878114
  price 1000773720000000000
  borrowValueUsd 50973.727373295803278736
  collateralValueUsd 0.000613041613531098
  isCollateralCappedByCash false
  initialData end
  repaid 50921883336061481947598
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 12
  gasUsedToApprove 25746
  gasUsedToRedeem 0
  asset end
  asset start 4
  initialData start
  symbol vUSDC
  vtoken 0xecA88125a5ADbe82614ffC12D0DB554E2e2867C8
  collateralFactor 800000000000000000
  collateralAmount 358766824
  borrowAmount 567988636611363632257261
  cash 40995423278631310392585040
  exchangeRate 215004843382592409562485380
  price 1000460330000000000
  borrowValueUsd 568250.098820454941278097
  collateralValueUsd 0.077172113098279976
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 5
  initialData start
  symbol vUSDT
  vtoken 0xfD5840Cd36d94D7229439859C0112a4185BC0255
  collateralFactor 800000000000000000
  collateralAmount 2260569030
  borrowAmount 18456180601049713191612
  cash 167249425209188013677527166
  exchangeRate 217035732487126732238426137
  price 1000000000000000000
  borrowValueUsd 18456.180601049713191612
  collateralValueUsd 0.490624255263763564
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 6
  initialData start
  symbol vCAKE
  vtoken 0x86aC3974e2BD0d60825230fa6F355fF11409df5c
  collateralFactor 550000000000000000
  collateralAmount 965
  borrowAmount 0
  cash 934730615905852945604718
  exchangeRate 246614296373284591063478232
  price 5250000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.000001249409679000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 7
  initialData start
  symbol vTUSD
  vtoken 0x08CEB3F4a7ed3500cA0982bcd0FC7816688084c3
  collateralFactor 800000000000000000
  collateralAmount 0
  borrowAmount 567637659718080460924470
  cash 170322575017834871677866389
  exchangeRate 204113187854884466431952156
  price 1000000000000000000
  borrowValueUsd 567637.659718080460924470
  collateralValueUsd 0.000000000000000000
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 8
  initialData start
  symbol vDOT
  vtoken 0x1610bc33319e9398de5f57B33a5b184c806aD217
  collateralFactor 600000000000000000
  collateralAmount 13293051
  borrowAmount 0
  cash 1290978535920025293841317
  exchangeRate 211386371467589062910113234
  price 10670000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.029982377943373876
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 9
  initialData start
  symbol vADA
  vtoken 0x9A0AF7FDb2065Ce470D72664DE73cAE409dA28Ec
  collateralFactor 600000000000000000
  collateralAmount 841125259
  borrowAmount 0
  cash 17527718238816535389170835
  exchangeRate 201470313645614750255556850
  price 631365700000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.106992348878908816
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 10
  initialData start
  symbol vMATIC
  vtoken 0x5c9476FcD6a4F9a3654139721c949c2233bBbBc8
  collateralFactor 600000000000000000
  collateralAmount 2733010282
  borrowAmount 0
  cash 2534808075149721012286791
  exchangeRate 203378028283301843803274252
  price 841068440000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.467494639180149767
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 11
  initialData start
  symbol vAAVE
  vtoken 0x26DA28954763B92139ED49283625ceCAf52C6f94
  collateralFactor 550000000000000000
  collateralAmount 36403409
  borrowAmount 0
  cash 12197275182207762794244
  exchangeRate 202601019036947906273617883
  price 102780552200000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 0.758044371032253235
  isCollateralCappedByCash false
  initialData end
  repaid 0
  collateralVTokenGained 0
  collateralUnderlyingGained 0
  liquidationsParticipated 0
  gasUsedToApprove 0
  gasUsedToRedeem 0
  asset end
  asset start 12
  initialData start
  symbol vUST
  vtoken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  collateralFactor 800000000000000000
  collateralAmount 14941036834169855
  borrowAmount 0
  cash 286473686274
  exchangeRate 202255874138524
  price 443006860000000000000000000000
  borrowValueUsd 0.000000000000000000
  collateralValueUsd 126909.808228869839640000
  isCollateralCappedByCash true
  initialData end
  repaid 0
  collateralVTokenGained 625635581241565
  collateralUnderlyingGained 126538471376
  liquidationsParticipated 12
  gasUsedToApprove 0
  gasUsedToRedeem 73177
  asset end
  numberOfLiquidations 12
  liquidation start 0
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 25467159236203619793758
  collateralVTokenGained 312894180801642
  collateralUnderlyingGained 63284686050
  gasUsed 1371046
  postHealthFactor 0.888734247444888248
  repaidUsd 25486.863686647855258464
  seizedUsd 28035.550053096303000000
  liquidation end
  liquidation start 1
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 12733579618101809896879
  collateralVTokenGained 156447090400821
  collateralUnderlyingGained 31642343025
  gasUsed 491845
  postHealthFactor 0.888829616729877829
  repaidUsd 12743.431843323927629232
  seizedUsd 14017.775026548151500000
  liquidation end
  liquidation start 2
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 6366789809050945644561
  collateralVTokenGained 78223545200411
  collateralUnderlyingGained 15821171512
  gasUsed 491856
  postHealthFactor 0.888878086662883938
  repaidUsd 6371.715921662004542225
  seizedUsd 7008.887513052572320000
  liquidation end
  liquidation start 3
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 3183394904525432126159
  collateralVTokenGained 39111772600205
  collateralUnderlyingGained 7910585756
  gasUsed 491867
  postHealthFactor 0.888902521734372799
  repaidUsd 3185.857960830961543503
  seizedUsd 3504.443756526286160000
  liquidation end
  liquidation start 4
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 1591697452262756759202
  collateralVTokenGained 19555886300103
  collateralUnderlyingGained 3955292878
  gasUsed 491877
  postHealthFactor 0.888914789778931390
  repaidUsd 1592.928980415521499361
  seizedUsd 1752.221878263143080000
  liquidation end
  liquidation start 5
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 795848726131337683479
  collateralVTokenGained 9777943150051
  collateralUnderlyingGained 1977646439
  gasUsed 491888
  postHealthFactor 0.888920936489359054
  repaidUsd 796.464490207720022071
  seizedUsd 876.110939131571540000
  liquidation end
  liquidation start 6
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 397924363065709537862
  collateralVTokenGained 4888971575026
  collateralUnderlyingGained 988823219
  gasUsed 491898
  postHealthFactor 0.888924013024267492
  repaidUsd 398.232245103900738645
  seizedUsd 438.055469344282340000
  liquidation end
  liquidation start 7
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 198962181532854768931
  collateralVTokenGained 2444485787513
  collateralUnderlyingGained 494411609
  gasUsed 491909
  postHealthFactor 0.888925552087605026
  repaidUsd 199.116122551950369322
  seizedUsd 219.027734450637740000
  liquidation end
  liquidation start 8
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 99481090766386688344
  collateralVTokenGained 1222242893756
  collateralUnderlyingGained 247205804
  gasUsed 491919
  postHealthFactor 0.888926321818364736
  repaidUsd 99.558061275934457052
  seizedUsd 109.513867003815440000
  liquidation end
  liquidation start 9
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 49740545383193344172
  collateralVTokenGained 611121446878
  collateralUnderlyingGained 123602902
  gasUsed 491931
  postHealthFactor 0.888926706733532350
  repaidUsd 49.779030637967228526
  seizedUsd 54.756933501907720000
  liquidation end
  liquidation start 10
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 24870272691596672086
  collateralVTokenGained 305560723439
  collateralUnderlyingGained 61801451
  gasUsed 491941
  postHealthFactor 0.888926899203564976
  repaidUsd 24.889515318983614263
  seizedUsd 27.378466750953860000
  liquidation end
  liquidation start 11
  repaySymbol vBUSD
  collateralSymbol vUST
  repayVToken 0x95c78222B3D6e262426483D42CfA53685A67Ab9D
  collateralVToken 0x78366446547D062f45b4C0f320cDaa6d710D87bb
  repayAmount 12435136345839032165
  collateralVTokenGained 152780361720
  collateralUnderlyingGained 30900725
  gasUsed 491952
  postHealthFactor 0.888926995441693728
  repaidUsd 12.444757659532534740
  seizedUsd 13.689233153973500000
  liquidation end
  gasUsage start
  approves 25746
  liquidations 6781929
  redeems 73177
  total 6880852
  gasUsage end
  gasPrice 5000000000
  chainCoinPrice 307500000000000000000
  gasFeeUsd 10.579309950000000000
  repaidUsd 50961.282615636259437410
  seizedUsd 56057.410873481639360000
  profitUsd 5085.548947895379922590
  strategyRunReport end
  Tests case end
        "#;

        let result = parse_logs(logs);

        println!("{:?}", result);

        assert_eq!(result.repeat.assets.len(), 2);
        assert_eq!(
            result.repeat.liquidations[0].collateral_v_token_gained,
            U256::from_dec_str("11802590948156").unwrap(),
        );
        // assert_eq!(
        //     repeat.collateral_v_token_gained_total,
        //     U256::from_dec_str("3437968650492").unwrap()
        // );
        // assert_eq!(repeat.redeem.gas_used_for_redeem, 62355);
        // assert_eq!(
        //     repeat.repay_token_price,
        //     U256::from_dec_str("1000000000000000000").unwrap()
        // );
        // assert_eq!(repeat.capped_by_collateral, None);

        // // Assertions for upToCloseFactorLiquidation
        // let up_to_close_factor = &result.up_to_close_factor.parsed;
        // assert_eq!(
        //     up_to_close_factor.repay_amount_total,
        //     U256::from_dec_str("16642946542590932675453").unwrap()
        // );
        // assert_eq!(
        //     up_to_close_factor.collateral_v_token_gained_total,
        //     U256::from_dec_str("3437968650492").unwrap()
        // );
        // assert_eq!(up_to_close_factor.redeem.gas_used_for_redeem, 87355);
        // assert_eq!(
        //     up_to_close_factor.collateral_token_price,
        //     U256::from_dec_str("26625000000000000000").unwrap()
        // );
        // assert_eq!(up_to_close_factor.capped_by_collateral, Some(false));

        // // Assertions for drainLiquidation
        // let drain = &result.drain.parsed;
        // assert_eq!(
        //     drain.repay_amount_total,
        //     U256::from_dec_str("16704439946681283418536").unwrap()
        // );
        // assert_eq!(
        //     drain.collateral_v_token_gained_total,
        //     U256::from_dec_str("3450671472972").unwrap()
        // );
        // assert_eq!(drain.redeem.gas_used_for_redeem, 87355);
        // assert_eq!(
        //     drain.chain_coin_price,
        //     U256::from_dec_str("26625000000000000000").unwrap()
        // );
        // assert_eq!(drain.capped_by_collateral, Some(false));

        // // Ensure there are multiple liquidation calls in the drainLiquidation case
        // assert_eq!(drain.liquidation_calls.len(), 2);
        // assert_eq!(
        //     drain.liquidation_calls[0].repay_amount,
        //     U256::from_dec_str("122986808185542410628").unwrap()
        // );
        // assert_eq!(
        //     drain.liquidation_calls[1].repay_amount,
        //     U256::from_dec_str("16581453138495741007908").unwrap()
        // );
    }
}
