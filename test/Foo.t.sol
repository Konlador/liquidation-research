// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.6.2;
pragma experimental ABIEncoderV2;

import { Test } from "forge-std/src/Test.sol";
import "forge-std/src/Test.sol";
import { console } from "forge-std/src/console.sol";

import { ComptrollerInterface } from "../src/Comptroller/ComptrollerInterface.sol";
import { VAIControllerInterface } from "../src/Tokens/VAI/VAIControllerInterface.sol";
import { VBep20Interface, VTokenInterface } from "../src/Tokens/VTokens/VTokenInterfaces.sol";
import { EIP20Interface } from "../src/Tokens/EIP20Interface.sol";
import { ComptrollerErrorReporter } from "../src/Utils/ErrorReporter.sol";
import "../src/Oracle/PriceOracle.sol";
import { ExponentialNoError } from "../src/Utils/ExponentialNoError.sol";
import { Liquidator } from "../src/Liquidator/Liquidator.sol";

contract LiquidationTest is Test, ExponentialNoError {
    using stdStorage for StdStorage;

    string QUICKNODE_RPC_URL = vm.envString("QUICKNODE_RPC_URL");
    // address venusLiquidator = 0x0870793286aaDA55D39CE7f82fb2766e8004cF43;
    ComptrollerInterface comptroller = ComptrollerInterface(0xfD36E2c2a6789Db23113685031d7F16329158384);
    // EIP20Interface usdt = EIP20Interface(0x55d398326f99059fF775485246999027B3197955);
    address myAddress = 0xBADc0De000000000000000000000000000000000;

    mapping(string => StrategyRunReport) public reports;
    uint256 public gasPrice;

    function setUp() public virtual { }

    function testLiquidations() external {
        bytes32 txHashLiquidation = vm.envBytes32("TX_HASH");
        VTokenInterface repayVToken = VTokenInterface(vm.envAddress("REPAY_V_TOKEN"));
        address borrower = vm.envAddress("BORROWER");
        uint256 repayAmount = vm.envUint("REPAY_AMOUNT");
        VTokenInterface collateralVToken = VTokenInterface(vm.envAddress("COLLATERAL_V_TOKEN"));
        uint256 expectedSeize = vm.envUint("EXPECTED_SEIZE");
        gasPrice = vm.envUint("GAS_PRICE");
        assertTrue(gasPrice > 0, "gasPrice is not set");

        uint256 txFork = vm.createSelectFork(QUICKNODE_RPC_URL, txHashLiquidation);
        vm.createSelectFork(QUICKNODE_RPC_URL, block.number + 1);
        uint256 nextBlockTime = block.timestamp; // Sometimes the time diff is not 3s (30935497-30935498 4s)
        vm.selectFork(txFork);

        // Important, because the default is the previous block metadata
        vm.roll(block.number + 1);
        vm.warp(nextBlockTime);
        assertTrue(block.number < 31_302_048, "not in istanbul");
        //assertTrue(block.number >= 31_302_048 && block.timestamp < 1_705_996_800, "not in berlin");
        // shanghai from 35490444 block
        // assertTrue(block.timestamp >= 1_705_996_800 && block.timestamp < 1_718_863_500, "not in shanghai");
        // assertTrue(block.timestamp >= 1_718_863_500, "not in cancun"); // 39769787 block

        // VTokenInterface(0x882C173bC7Ff3b7786CA16dfeD3DFFfb9Ee7847B).accrueInterest();
        // VTokenInterface(0x95c78222B3D6e262426483D42CfA53685A67Ab9D).accrueInterest();
        // VTokenInterface(0xfD5840Cd36d94D7229439859C0112a4185BC0255).accrueInterest();

        uint256 freshSnapshotId = vm.snapshot();

        assertAssumtions(collateralVToken);

        startHoax(myAddress);

        {
            address liquidator = getLiquidatorContract();
            console.log("liquidator", liquidator);
            if (liquidator != address(0)) {
                // Adjust expected seize to take into account the treasury cut
                // The expected seize we parsed from the events is before the treasury cut
                expectedSeize -= expectedSeize / 22; // (seizedAmount * treasuryPercentMantissa) / totalIncentive;
            }
        }

        logHealthFactor(borrower, repayVToken, collateralVToken);

        // {
        //     uint256 treasuryPercent = comptroller.treasuryPercent();
        //     console.log("treasuryPercent", treasuryPercent);
        // }

        // #1 Repeat
        bool needToClaimXvs = repeatLiquidation(borrower, repayVToken, collateralVToken, repayAmount, expectedSeize);
        vm.revertTo(freshSnapshotId);

        if (needToClaimXvs) {
            // claim xvs
            freshSnapshotId = vm.snapshot();

            address[] memory holders = new address[](1);
            holders[0] = borrower;
            address[] memory vtokens = new address[](1);
            vtokens[0] = address(repayVToken);
            comptroller.claimVenus(holders, vtokens, true, false, true);
        }

        // #2 Up to close factor
        upToCloseFactorLiquidation(borrower, repayVToken, collateralVToken);
        vm.revertTo(freshSnapshotId);

        // #3 Drain
        drainLiquidation(borrower, repayVToken, collateralVToken);
        vm.revertTo(freshSnapshotId);

        // #4 Largest borrow (upgraded Up to close factor)
        largestBorrow(borrower);
        vm.revertTo(freshSnapshotId);

        // #5 Drain same token (simpler Drain case)
        drainSameToken(borrower);
        vm.revertTo(freshSnapshotId);

        // #6 Largest collateral factor first
        // This should work better than smallest
        largestCollateralFactorFirst(borrower, repayVToken);
        vm.revertTo(freshSnapshotId);

        // #7 Smallest collateral factor first
        smallestCollateralFactorFirst(borrower, repayVToken);
        vm.revertTo(freshSnapshotId);
    }

    struct StrategyRunReport {
        string initialHealthFactor; // Without any accrue
        string finalHealthFactor;
        LiquidationReport[] liquidations;
        AssetReport[] assets;
        GasUsage gasUsage;
        uint256 gasPrice;
        uint256 chainCoinPrice;
        uint256 gasFeeUsd; // gasUsage.total * gasPrice * chainCoinPrice
        uint256 repaidUsd; // sum by asset the sum of repaid * price
        uint256 seizedUsd;
        int256 profitUsd;
    }

    struct LiquidationReport {
        string repaySymbol;
        string collateralSymbol;
        VTokenInterface repayVToken;
        VTokenInterface collateralVToken;
        uint256 repayAmount;
        uint256 collateralVTokenGained;
        uint256 collateralUnderlyingGained; // Includes redeem fees
        uint256 gasUsed;
        string postHealthFactor;
        // Derivatives
        uint256 repaidUsd; // repayAmount * price
        uint256 seizedUsd; // collateralUnderlyingGained * price
    }

    struct AssetReport {
        AssetData initialData;
        uint256 repaid;
        uint256 collateralVTokenGained;
        uint256 collateralUnderlyingGained;
        uint256 liquidationsParticipated;
        uint256 gasUsedToApprove;
        uint256 gasUsedToRedeem;
    }

    struct AssetData {
        string symbol;
        VTokenInterface vtoken;
        uint256 collateralFactor;
        uint256 collateralAmount;
        uint256 borrowAmount;
        uint256 cash;
        uint256 exchangeRate;
        uint256 price;
        // Derivatives:
        uint256 borrowValue; // borrowAmount * price
        uint256 collateralValue; // min(collateralAmount * exchangeRate, cash) * price
        bool isCollateralCappedByCash;
    }

    struct GasUsage {
        uint256 approves;
        uint256 liquidations;
        uint256 redeems;
        uint256 total;
    }

    function logStrategyReport(StrategyRunReport storage report) private view {
        console.log("strategyRunReport start");

        console.log("initialHealthFactor", report.initialHealthFactor);
        console.log("finalHealthFactor", report.finalHealthFactor);

        for (uint256 i = 0; i < report.assets.length; i++) {
            AssetReport memory b = report.assets[i];
            console.log("asset start", i);
            console.log("initialData start");
            logAsset(b.initialData);
            console.log("initialData end");
            console.log("repaid", b.repaid);
            console.log("collateralVTokenGained", b.collateralVTokenGained);
            console.log("collateralUnderlyingGained", b.collateralUnderlyingGained);
            console.log("liquidationsParticipated", b.liquidationsParticipated);
            console.log("gasUsedToApprove", b.gasUsedToApprove);
            console.log("gasUsedToRedeem", b.gasUsedToRedeem);
            console.log("asset end");
        }

        console.log("numberOfLiquidations", report.liquidations.length);
        for (uint256 i = 0; i < report.liquidations.length; i++) {
            LiquidationReport memory l = report.liquidations[i];
            console.log("liquidation start", i);
            console.log("repaySymbol", l.repaySymbol);
            console.log("collateralSymbol", l.collateralSymbol);
            console.log("repayVToken", address(l.repayVToken));
            console.log("collateralVToken", address(l.collateralVToken));
            console.log("repayAmount", l.repayAmount);
            console.log("collateralVTokenGained", l.collateralVTokenGained);
            console.log("collateralUnderlyingGained", l.collateralUnderlyingGained);
            console.log("gasUsed", l.gasUsed);
            console.log("postHealthFactor", l.postHealthFactor);
            console.log("repaidUsd", ratioToString(l.repaidUsd, 1e18));
            console.log("seizedUsd", ratioToString(l.seizedUsd, 1e18));
            console.log("liquidation end");
        }

        console.log("gasUsage start");
        console.log("approves", report.gasUsage.approves);
        console.log("liquidations", report.gasUsage.liquidations);
        console.log("redeems", report.gasUsage.redeems);
        console.log("total", report.gasUsage.total);
        console.log("gasUsage end");

        console.log("gasPrice", report.gasPrice);
        console.log("chainCoinPrice", report.chainCoinPrice);
        console.log("gasFeeUsd", ratioToString(report.gasFeeUsd, 1e18));
        console.log("repaidUsd", ratioToString(report.repaidUsd, 1e18));
        console.log("seizedUsd", ratioToString(report.seizedUsd, 1e18));
        console.log("profitUsd", ratioToStringSigned(report.profitUsd, 1e18));

        console.log("strategyRunReport end");
    }

    struct RepeatLiquidationVars {
        AssetData repayAsset;
        AssetData collateralAsset;
        uint256 collateralUnderlyingGained;
    }

    function repeatLiquidation(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        uint256 repayAmount,
        uint256 expectedSeize
    )
        private
        returns (bool needToClaimXvs)
    {
        console.log("");
        console.log("Tests case: repeatLiquidation");
        StrategyRunReport storage report = startStrategy("repeatLiquidation", borrower);

        RepeatLiquidationVars memory vars;
        (vars.repayAsset,) = getAsset(borrower, repayVToken);
        (vars.collateralAsset,) = getAsset(borrower, collateralVToken);

        {
            AssetData[] memory assets;
            if (repayVToken == collateralVToken) {
                assets = new AssetData[](1);
                assets[0] = vars.repayAsset;
            } else {
                assets = new AssetData[](2);
                assets[0] = vars.repayAsset;
                assets[1] = vars.collateralAsset;
            }
            storeAssets(report, assets);
        }

        dealAndMaybeApproveRepayToken(borrower, repayVToken, report);
        uint256 snapshot = vm.snapshot();
        (uint256 errorCode, uint256 collateralVTokenGained, uint256 gasUsed) =
            callLiquidateWithResult(borrower, repayVToken, collateralVToken, repayAmount);
        if (errorCode != 0) {
            if (collateralVToken != VTokenInterface(0x151B1e2635A717bcDc836ECd6FbB62B674FE3E1D)) {
                revert("Liquidation failed");
            }
            // Try to claim xvs and do the liquidation again
            vm.revertTo(snapshot);

            // Claim xvs
            {
                address[] memory holders = new address[](1);
                holders[0] = borrower;
                address[] memory vtokens = new address[](1);
                vtokens[0] = address(repayVToken);
                comptroller.claimVenus(holders, vtokens, true, false, true);
            }

            // Do the liquidation
            (collateralVTokenGained, gasUsed) = callLiquidate(borrower, repayVToken, collateralVToken, repayAmount);
            needToClaimXvs = true;
        }
        assertEq(collateralVTokenGained, expectedSeize, "expected seize does not match");

        vars.collateralUnderlyingGained = simulateRedeem(collateralVToken, collateralVTokenGained);
        report.liquidations.push(
            LiquidationReport({
                repaySymbol: vars.repayAsset.symbol,
                repayVToken: vars.repayAsset.vtoken,
                collateralSymbol: vars.collateralAsset.symbol,
                collateralVToken: vars.collateralAsset.vtoken,
                repayAmount: repayAmount,
                collateralVTokenGained: collateralVTokenGained,
                collateralUnderlyingGained: vars.collateralUnderlyingGained,
                gasUsed: gasUsed,
                postHealthFactor: getHealthFactor(borrower),
                repaidUsd: repayAmount * vars.repayAsset.price / 1e18,
                seizedUsd: vars.collateralUnderlyingGained * vars.collateralAsset.price / 1e18
            })
        );

        finishStrategy(borrower, report);
        console.log("Tests case end");
    }

    function upToCloseFactorLiquidation(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken
    )
        private
    {
        console.log("");
        console.log("Tests case: upToCloseFactorLiquidation");
        StrategyRunReport storage report = startStrategy("upToCloseFactorLiquidation", borrower);

        (AssetData memory repayAsset,) = getAsset(borrower, repayVToken);
        (AssetData memory collateralAsset,) = getAsset(borrower, collateralVToken);
        AssetData[] memory assets;
        if (repayVToken == collateralVToken) {
            assets = new AssetData[](1);
            assets[0] = repayAsset;
        } else {
            assets = new AssetData[](2);
            assets[0] = repayAsset;
            assets[1] = collateralAsset;
        }
        storeAssets(report, assets);

        dealAndMaybeApproveRepayToken(borrower, repayVToken, report);
        upToCloseFactorLiquidationInternal(borrower, repayAsset, collateralAsset, report);
        // assertGe(
        //     collateralVTokenGained,
        //     originalSeizeAmount,
        //     "Expected to get greater or equal collateral than the original liquidation"
        // );

        finishStrategy(borrower, report);
        console.log("Tests case end");
    }

    function upToCloseFactorLiquidationInternal(
        address borrower,
        AssetData memory repayAsset,
        AssetData memory collateralAsset,
        StrategyRunReport storage report
    )
        private
    {
        (uint256 repayAmount,) = findMaxRepayAmount(repayAsset.vtoken, collateralAsset.vtoken, borrower);
        repayAmount = findSmallestEffectiveRepayAmount(repayAsset.vtoken, collateralAsset.vtoken, repayAmount, borrower);
        if (repayAmount == 0) {
            revert("No collateral to be gained!!!");
        }
        callLiquidateIfCoversGas(borrower, repayAsset, collateralAsset, repayAmount, report);
    }

    function drainLiquidation(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken
    )
        private
    {
        console.log("");
        console.log("Tests case: drainLiquidation");
        StrategyRunReport storage report = startStrategy("drainLiquidation", borrower);

        (AssetData memory repayAsset,) = getAsset(borrower, repayVToken);
        (AssetData memory collateralAsset,) = getAsset(borrower, collateralVToken);
        AssetData[] memory assets;
        if (repayVToken == collateralVToken) {
            assets = new AssetData[](1);
            assets[0] = repayAsset;
        } else {
            assets = new AssetData[](2);
            assets[0] = repayAsset;
            assets[1] = collateralAsset;
        }
        storeAssets(report, assets);

        dealAndMaybeApproveRepayToken(borrower, repayVToken, report);
        drainLiquidationInternal(borrower, repayAsset, collateralAsset, report);

        finishStrategy(borrower, report);
        console.log("Tests case end");
    }

    function drainLiquidationInternal(
        address borrower,
        AssetData memory repayAsset,
        AssetData memory collateralAsset,
        StrategyRunReport storage report
    )
        private
        returns (bool cappedByCollateral)
    {
        uint256 repayAmount;
        while (true) {
            (repayAmount, cappedByCollateral) = findMaxRepayAmount(repayAsset.vtoken, collateralAsset.vtoken, borrower);
            if (repayAmount == 0 || cappedByCollateral) break;
            repayAmount = findRepayAmountThatKeepsBorrowerLiquidatable(
                repayAsset.vtoken, borrower, collateralAsset.vtoken, repayAmount
            );
            repayAmount =
                findSmallestEffectiveRepayAmount(repayAsset.vtoken, collateralAsset.vtoken, repayAmount, borrower);
            if (repayAmount == 0) break;

            bool profitCoversGas = callLiquidateIfCoversGas(borrower, repayAsset, collateralAsset, repayAmount, report);
            if (!profitCoversGas) {
                break;
            }
        }

        // (Maybe) last liquidation
        (repayAmount, cappedByCollateral) = findMaxRepayAmount(repayAsset.vtoken, collateralAsset.vtoken, borrower);
        repayAmount = findSmallestEffectiveRepayAmount(repayAsset.vtoken, collateralAsset.vtoken, repayAmount, borrower);
        if (repayAmount == 0) {
            return cappedByCollateral;
        }
        callLiquidateIfCoversGas(borrower, repayAsset, collateralAsset, repayAmount, report);
    }

    function largestBorrow(address borrower) private {
        console.log("");
        console.log("Tests case: largestBorrow");
        StrategyRunReport storage report = startStrategy("largestBorrow", borrower);

        AssetData[] memory assets = getAssets(borrower);
        storeAssets(report, assets);

        (AssetData memory repayAsset, AssetData memory collateralAsset) = pickRepayAndCollateralAssets(assets);
        console.log("repayVToken", address(repayAsset.vtoken));
        console.log("repay accrualBlockNumber", repayAsset.vtoken.accrualBlockNumber());
        console.log("collateralVToken", address(collateralAsset.vtoken));
        console.log("collateral accrualBlockNumber", collateralAsset.vtoken.accrualBlockNumber());

        dealAndMaybeApproveRepayToken(borrower, repayAsset.vtoken, report);
        upToCloseFactorLiquidationInternal(borrower, repayAsset, collateralAsset, report);

        finishStrategy(borrower, report);
        console.log("Tests case end");
    }

    function drainSameToken(address borrower) private {
        console.log("");
        console.log("Tests case: drainSameToken");
        StrategyRunReport storage report = startStrategy("drainSameToken", borrower);

        AssetData[] memory assets = getAssets(borrower);
        storeAssets(report, assets);
        logAssets(assets);

        assets = pickAssetsThatAreBorrowedAndStaked(assets);
        for (uint256 i = 0; i < assets.length; i++) {
            AssetData memory asset = assets[i];
            VTokenInterface repayVToken = assets[i].vtoken;
            console.log("token", asset.symbol);
            dealAndMaybeApproveRepayToken(borrower, repayVToken, report);
            drainLiquidationInternal(borrower, asset, asset, report);
        }

        if (assets.length == 0) {
            console.log("there are no tokens that are both borrowed and staked");
        }
        finishStrategy(borrower, report);
        console.log("Tests case end");
    }

    function largestCollateralFactorFirst(address borrower, VTokenInterface repayVToken) private {
        console.log("");
        console.log("Tests case: largestCollateralFactorFirst");
        StrategyRunReport storage report = startStrategy("fromLargestCollateralFactor", borrower);
        orderedCollateralFactor(borrower, repayVToken, report, CollateralFactorOrder.HighestFirst);
        finishStrategy(borrower, report);
        console.log("Tests case end");
    }

    function smallestCollateralFactorFirst(address borrower, VTokenInterface repayVToken) private {
        console.log("");
        console.log("Tests case: smallestCollateralFactorFirst");
        StrategyRunReport storage report = startStrategy("fromLargestCollateralFactor", borrower);
        orderedCollateralFactor(borrower, repayVToken, report, CollateralFactorOrder.LowestFirst);
        finishStrategy(borrower, report);
        console.log("Tests case end");
    }

    struct MultiCollateralVars {
        uint256 cycleIndex;
        AssetData[] assets;
        AssetData repayAsset;
        AssetData currentCollateralAsset;
        AssetData[] collateralAssets;
        VTokenInterface[] processedCollaterals;
    }

    function orderedCollateralFactor(
        address borrower,
        VTokenInterface repayVToken,
        StrategyRunReport storage report,
        CollateralFactorOrder order
    )
        private
    {
        MultiCollateralVars memory vars;
        vars.processedCollaterals = new VTokenInterface[](0);

        vars.assets = getAssets(borrower);
        storeAssets(report, vars.assets);
        (vars.repayAsset, vars.collateralAssets) =
            groupAssets(vars.assets, repayVToken, vars.processedCollaterals, order);
        console.log("repayAsset:");
        logAsset(vars.repayAsset);
        console.log("collateralAssets:");
        logAssets(vars.collateralAssets);

        dealAndMaybeApproveRepayToken(borrower, vars.repayAsset.vtoken, report);

        for (vars.cycleIndex = 0;; vars.cycleIndex++) {
            console.log("");
            console.log("cycleIndex", vars.cycleIndex);
            logHealthFactor(borrower, repayVToken, repayVToken);

            vars.assets = getAssets(borrower);
            (vars.repayAsset, vars.collateralAssets) =
                groupAssets(vars.assets, repayVToken, vars.processedCollaterals, order);

            console.log("numberOfCollateralAssets", vars.collateralAssets.length);

            vars.currentCollateralAsset = vars.collateralAssets[0];
            console.log("currentCollateral", vars.currentCollateralAsset.symbol);

            if (vars.collateralAssets.length <= 1) {
                break;
            }

            AssetData memory largestCollateralAsset = vars.collateralAssets[1];
            for (uint256 i = 2; i < vars.collateralAssets.length; i++) {
                if (vars.collateralAssets[i].collateralValue > largestCollateralAsset.collateralValue) {
                    largestCollateralAsset = vars.collateralAssets[i];
                }
            }
            if (vars.collateralAssets[0].collateralValue > largestCollateralAsset.collateralValue) {
                // try to drain this collateral, if in the end we are capped by collateral, then we come here and
                // rethink
                uint256 snapshot = vm.snapshot();
                console.log("other collaterals are smaller, trying drain strategy");
                bool cappedByCollateral =
                    drainLiquidationInternal(borrower, vars.repayAsset, vars.collateralAssets[0], report);
                if (cappedByCollateral) {
                    vm.revertTo(snapshot);
                    // TODO: revert the report as well
                    revert("unhandeled case, study this");
                }
                return; // We are done, used up all borrow
            }

            (uint256 largestCollateralRepayAmount,) =
                findMaxRepayAmount(repayVToken, largestCollateralAsset.vtoken, borrower);
            largestCollateralRepayAmount = findSmallestEffectiveRepayAmount(
                repayVToken, largestCollateralAsset.vtoken, largestCollateralRepayAmount, borrower
            );

            // Ieskant einamo repayAmount, norim po jo galeti likviduoti repayAmount didziausiame uzstate:
            // freeRepayAmount = borrowSize - maxEffectiveRepayAmount/closeFactor
            // maxEffectiveRepayAmount - max(0, einamo.repayAmount - freeRepayAmount)*closeFactor

            uint256 freeRepayAmount = vars.repayAsset.borrowAmount - largestCollateralRepayAmount * 2;

            (uint256 repayAmount) = findRepayAmountThatKeepsBorrowerLiquidatableWithMultiCollateralSupport(
                borrower,
                repayVToken,
                vars.currentCollateralAsset.vtoken,
                largestCollateralAsset.vtoken,
                largestCollateralRepayAmount,
                freeRepayAmount
            );
            repayAmount =
                findSmallestEffectiveRepayAmount(repayVToken, vars.currentCollateralAsset.vtoken, repayAmount, borrower);
            if (repayAmount == 0) {
                vars.processedCollaterals =
                    appendProcessedCollateral(vars.processedCollaterals, vars.currentCollateralAsset.vtoken);
                continue;
            }

            bool profitCoversGas =
                callLiquidateIfCoversGas(borrower, vars.repayAsset, vars.currentCollateralAsset, repayAmount, report);
            if (!profitCoversGas) {
                vars.processedCollaterals =
                    appendProcessedCollateral(vars.processedCollaterals, vars.currentCollateralAsset.vtoken);
                continue;
            }
        }

        console.log("only one collateral left, doing drain");
        // Usually it should only do a single liquidation, but in the rare case where
        // there is one overwhelming collateral token it may do multiple liquidations.
        drainLiquidationInternal(borrower, vars.repayAsset, vars.collateralAssets[0], report);
    }

    function storeAssets(StrategyRunReport storage report, AssetData[] memory assets) private {
        for (uint256 i = 0; i < assets.length; i++) {
            report.assets.push(
                AssetReport({
                    initialData: assets[i],
                    repaid: 0, // Look sumRepays
                    collateralVTokenGained: 0, // Look redeemCollaterals
                    collateralUnderlyingGained: 0, // Look redeemCollaterals
                    liquidationsParticipated: 0, // Look sumRepays
                    gasUsedToApprove: 0, // Look maybeApproveRepayToken
                    gasUsedToRedeem: 0 // Look redeemCollaterals
                 })
            );
        }
    }

    function startStrategy(
        string memory strategyName,
        address borrower
    )
        private
        returns (StrategyRunReport storage report)
    {
        report = reports[strategyName];
        report.initialHealthFactor = getHealthFactor(borrower);
        report.chainCoinPrice = getChainCoinPrice();
        report.gasPrice = gasPrice;

        return report;
    }

    function finishStrategy(address borrower, StrategyRunReport storage report) private {
        report.finalHealthFactor = getHealthFactor(borrower);
        sumRepays(report);
        redeemCollaterals(report);
        calculateGas(report);
        calculateProfits(report);

        console.log("");
        logStrategyReport(report);
    }

    function sumRepays(StrategyRunReport storage report) private {
        for (uint256 i = 0; i < report.liquidations.length; i += 1) {
            LiquidationReport storage liquidation = report.liquidations[i];

            AssetReport storage rapayAsset = findAsset(report, liquidation.repayVToken);
            rapayAsset.repaid += liquidation.repayAmount;
            rapayAsset.liquidationsParticipated += 1;

            if (liquidation.repayVToken != liquidation.collateralVToken) {
                AssetReport storage collateralAsset = findAsset(report, liquidation.collateralVToken);
                collateralAsset.liquidationsParticipated += 1;
            }
        }
    }

    function redeemCollaterals(StrategyRunReport storage report) private {
        for (uint256 i = 0; i < report.assets.length; i += 1) {
            AssetReport storage asset = report.assets[i];
            asset.collateralVTokenGained = asset.initialData.vtoken.balanceOf(myAddress);
            if (asset.collateralVTokenGained == 0) continue;

            (asset.collateralUnderlyingGained, asset.gasUsedToRedeem) =
                redeemCollateral(asset.initialData.vtoken, asset.collateralVTokenGained);
        }
    }

    function calculateGas(StrategyRunReport storage report) private {
        for (uint256 i = 0; i < report.assets.length; i += 1) {
            AssetReport storage asset = report.assets[i];
            bool thereAreLiquidations = false;
            for (uint256 j = 0; j < report.liquidations.length; ++j) {
                LiquidationReport storage liquidation = report.liquidations[j];
                if (
                    liquidation.repayVToken == asset.initialData.vtoken
                        || liquidation.collateralVToken == asset.initialData.vtoken
                ) {
                    thereAreLiquidations = true;
                    break;
                }
            }
            if (!thereAreLiquidations) {
                asset.gasUsedToApprove = 0;
                asset.gasUsedToRedeem = 0;
                continue;
            }
            report.gasUsage.approves += asset.gasUsedToApprove;
            report.gasUsage.redeems += asset.gasUsedToRedeem;
        }

        for (uint256 i = 0; i < report.liquidations.length; i += 1) {
            report.gasUsage.liquidations += report.liquidations[i].gasUsed;
        }

        report.gasUsage.total += report.gasUsage.approves;
        report.gasUsage.total += report.gasUsage.liquidations;
        report.gasUsage.total += report.gasUsage.redeems;

        report.gasFeeUsd = report.gasUsage.total * report.gasPrice * report.chainCoinPrice / 1e18;
    }

    function calculateProfits(StrategyRunReport storage report) private {
        for (uint256 i = 0; i < report.assets.length; i += 1) {
            AssetReport memory asset = report.assets[i];
            report.repaidUsd += asset.repaid * asset.initialData.price / 1e18;
            report.seizedUsd += asset.collateralUnderlyingGained * asset.initialData.price / 1e18;
        }

        report.profitUsd = int256(report.seizedUsd) - int256(report.repaidUsd) - int256(report.gasFeeUsd);
    }

    function dealAndMaybeApproveRepayToken(
        address borrower,
        VTokenInterface repayVToken,
        StrategyRunReport storage report
    )
        private
    {
        uint256 borrowBalance = findBorrowBalance(repayVToken, borrower);
        dealUnderlyingTokens(repayVToken, borrowBalance + 1);
        maybeApproveRepayToken(repayVToken, borrowBalance, report);
    }

    // Returns the collateral token you'd get for redeeming the amount
    function simulateRedeem(VTokenInterface vToken, uint256 amount) private returns (uint256) {
        uint256 snapshotId = vm.snapshot();

        uint256 collateralBalanceBefore;
        EIP20Interface collateralUnderlyingToken;
        bool isCollateralChainCoin = isVTokenChainCoin(vToken);
        if (isCollateralChainCoin) {
            collateralBalanceBefore = myAddress.balance;
        } else {
            collateralUnderlyingToken = EIP20Interface(vToken.underlying());
            collateralBalanceBefore = collateralUnderlyingToken.balanceOf(myAddress);
        }
        uint256 success = VBep20Interface(address(vToken)).redeem(amount);
        assertEq(success, 0, "Redeem failed");
        uint256 collateralBalanceAfter;
        if (isCollateralChainCoin) collateralBalanceAfter = myAddress.balance;
        else collateralBalanceAfter = collateralUnderlyingToken.balanceOf(myAddress);
        uint256 collateralUnderlyingGained = collateralBalanceAfter - collateralBalanceBefore;

        vm.revertTo(snapshotId);

        return collateralUnderlyingGained;
    }

    function appendProcessedCollateral(
        VTokenInterface[] memory arr,
        VTokenInterface newItem
    )
        private
        pure
        returns (VTokenInterface[] memory)
    {
        VTokenInterface[] memory newArr = new VTokenInterface[](arr.length + 1);
        for (uint256 i = 0; i < arr.length; i++) {
            newArr[i] = arr[i];
        }
        newArr[arr.length] = newItem;
        return newArr;
    }

    enum CollateralFactorOrder {
        LowestFirst,
        HighestFirst
    }

    function isInProcessed(
        VTokenInterface vtoken,
        VTokenInterface[] memory processedCollaterals
    )
        private
        pure
        returns (bool)
    {
        for (uint256 i = 0; i < processedCollaterals.length; i++) {
            if (processedCollaterals[i] == vtoken) {
                return true;
            }
        }
        return false;
    }

    function groupAssets(
        AssetData[] memory assets,
        VTokenInterface repayVToken,
        VTokenInterface[] memory processedCollaterals,
        CollateralFactorOrder order
    )
        private
        pure
        returns (AssetData memory repayAsset, AssetData[] memory collateralAssets)
    {
        uint256 length = assets.length;
        uint256 collateralCount = 0;

        // First, separate the repay asset and count valid collateral assets
        for (uint256 i = 0; i < length; i++) {
            if (assets[i].vtoken == repayVToken) {
                repayAsset = assets[i];
            }
            if (assets[i].collateralValue > 0 && !isInProcessed(assets[i].vtoken, processedCollaterals)) {
                collateralCount++;
            }
        }

        // Initialize collateralAssets array
        collateralAssets = new AssetData[](collateralCount);
        uint256 collateralIndex = 0;

        for (uint256 i = 0; i < length; i++) {
            if (assets[i].collateralValue > 0 && !isInProcessed(assets[i].vtoken, processedCollaterals)) {
                collateralAssets[collateralIndex] = assets[i];
                collateralIndex++;
            }
        }

        // Sort collateralAssets
        for (uint256 i = 0; i < collateralCount; i++) {
            for (uint256 j = i + 1; j < collateralCount; j++) {
                bool shouldSwap = false;

                if (collateralAssets[i].collateralFactor == collateralAssets[j].collateralFactor) {
                    // For equal collateralFactor, sort by collateralValue ascending
                    shouldSwap = collateralAssets[i].collateralValue > collateralAssets[j].collateralValue;
                } else if (order == CollateralFactorOrder.LowestFirst) {
                    shouldSwap = collateralAssets[i].collateralFactor > collateralAssets[j].collateralFactor;
                } else {
                    shouldSwap = collateralAssets[i].collateralFactor < collateralAssets[j].collateralFactor;
                }

                if (shouldSwap) {
                    AssetData memory temp = collateralAssets[i];
                    collateralAssets[i] = collateralAssets[j];
                    collateralAssets[j] = temp;
                }
            }
        }

        return (repayAsset, collateralAssets);
    }

    function getHealthFactor(address borrower) private returns (string memory result) {
        uint256 snapshot = vm.snapshot();

        (uint256 borrowCapacity, uint256 borrowed) = getAccountLiquidity(borrower);
        result = ratioToString(borrowCapacity, borrowed);

        vm.revertTo(snapshot);
        return result;
    }

    function logHealthFactor(address borrower) private {
        logHealthFactor(borrower, new VTokenInterface[](0));
    }

    function logHealthFactor(address borrower, VTokenInterface vToken) private {
        VTokenInterface[] memory vTokens = new VTokenInterface[](1);
        vTokens[0] = vToken;
        logHealthFactor(borrower, vTokens);
    }

    function logHealthFactor(address borrower, VTokenInterface vToken1, VTokenInterface vToken2) private {
        VTokenInterface[] memory vTokens = new VTokenInterface[](2);
        vTokens[0] = vToken1;
        vTokens[1] = vToken2;
        logHealthFactor(borrower, vTokens);
    }

    function logHealthFactor(address borrower, VTokenInterface[] memory vTokensToAccrue) private {
        uint256 snapshot = vm.snapshot();

        for (uint256 i = 0; i < vTokensToAccrue.length; i++) {
            vTokensToAccrue[i].accrueInterest();
        }

        console.log("healthFactor", getHealthFactor(borrower));

        vm.revertTo(snapshot);
    }

    function ratioToString(uint256 numerator, uint256 denumerator) internal pure returns (string memory) {
        uint256 integerPart = numerator / denumerator;
        uint256 fractionPart = (numerator - (integerPart * denumerator)) * 1e18 / denumerator;

        string memory integerString = uintToString(integerPart);

        uint256 digits = 18;
        bytes memory buffer = new bytes(digits);
        uint256 value = fractionPart;
        while (value != 0) {
            digits -= 1;
            buffer[digits] = bytes1(uint8(48 + uint256(value % 10)));
            value /= 10;
        }
        while (digits != 0) {
            digits -= 1;
            buffer[digits] = bytes1("0");
        }
        string memory fractionString = string(buffer);

        return string(abi.encodePacked(integerString, ".", fractionString));
    }

    function ratioToStringSigned(int256 numerator, int256 denumerator) internal pure returns (string memory) {
        if (denumerator < 0) {
            revert("denumerator can not be negative");
        }
        if (numerator >= 0) {
            return ratioToString(uint256(numerator), uint256(denumerator));
        }

        string memory str = ratioToString(uint256(-numerator), uint256(denumerator));
        return string(abi.encodePacked("-", str));
    }

    function uintToString(uint256 value) internal pure returns (string memory) {
        if (value == 0) {
            return "0";
        }
        uint256 temp = value;
        uint256 digits;
        while (temp != 0) {
            digits++;
            temp /= 10;
        }
        bytes memory buffer = new bytes(digits);
        while (value != 0) {
            digits -= 1;
            buffer[digits] = bytes1(uint8(48 + uint256(value % 10)));
            value /= 10;
        }
        return string(buffer);
    }

    struct AccountLiquidityLocalVars {
        uint256 sumCollateral;
        uint256 sumBorrowPlusEffects;
        uint256 vTokenBalance;
        uint256 borrowBalance;
        uint256 exchangeRateMantissa;
        uint256 oraclePriceMantissa;
        Exp collateralFactor;
        Exp exchangeRate;
        Exp oraclePrice;
        Exp tokensToDenom;
    }

    function getAccountLiquidity(address account) private returns (uint256, uint256) {
        AccountLiquidityLocalVars memory vars; // Holds all our calculation results
        uint256 oErr;

        // For each asset the account is in
        address[] memory assets = comptroller.getAssetsIn(account);
        uint256 assetsCount = assets.length;
        for (uint256 i = 0; i < assetsCount; ++i) {
            VTokenInterface asset = VTokenInterface(assets[i]);

            // Read the balances and exchange rate from the vToken
            (oErr, vars.vTokenBalance, vars.borrowBalance, vars.exchangeRateMantissa) =
                asset.getAccountSnapshot(account);
            if (oErr != 0) {
                revert("SNAPSHOT_ERROR");
            }
            (, uint256 collateralFactorMantissa) = ComptrollerInterface(comptroller).markets(address(asset));
            vars.collateralFactor = Exp({ mantissa: collateralFactorMantissa });
            vars.exchangeRate = Exp({ mantissa: vars.exchangeRateMantissa });

            // Get the normalized price of the asset
            vars.oraclePriceMantissa = ComptrollerInterface(comptroller).oracle().getUnderlyingPrice(address(asset));
            if (vars.oraclePriceMantissa == 0) {
                revert("PRICE_ERROR");
            }
            vars.oraclePrice = Exp({ mantissa: vars.oraclePriceMantissa });

            // Pre-compute a conversion factor from tokens -> bnb (normalized price value)
            vars.tokensToDenom = mul_(mul_(vars.collateralFactor, vars.exchangeRate), vars.oraclePrice);

            // sumCollateral += tokensToDenom * vTokenBalance
            vars.sumCollateral = mul_ScalarTruncateAddUInt(vars.tokensToDenom, vars.vTokenBalance, vars.sumCollateral);

            // sumBorrowPlusEffects += oraclePrice * borrowBalance
            vars.sumBorrowPlusEffects =
                mul_ScalarTruncateAddUInt(vars.oraclePrice, vars.borrowBalance, vars.sumBorrowPlusEffects);
        }

        VAIControllerInterface vaiController = VAIControllerInterface(ComptrollerInterface(comptroller).vaiController());

        if (address(vaiController) != address(0)) {
            uint256 vaiDebt;

            bytes4 SELECTOR = bytes4(keccak256(bytes("getVAIRepayAmount(address)")));
            (bool success, bytes memory data) = address(vaiController).call(abi.encodeWithSelector(SELECTOR, account));
            if (success && data.length > 0) {
                console.log("data length returned", data.length);
                vaiDebt = abi.decode(data, (uint256));
            }

            // try vaiController.getVAIRepayAmount(account) returns (uint256 vaiDebtResult) {
            //     vaiDebt = vaiDebtResult;
            // } catch {
            //     // The getVAIRepayAmount got implemented only later
            // }

            if (vaiDebt > 0) {
                revert("watch this (2)");
                vars.sumBorrowPlusEffects = add_(vars.sumBorrowPlusEffects, vaiDebt);
            }
        }

        return (vars.sumCollateral, vars.sumBorrowPlusEffects);
    }

    function pickAssetsThatAreBorrowedAndStaked(AssetData[] memory assets) private pure returns (AssetData[] memory) {
        AssetData[] memory tempResult = new AssetData[](assets.length);
        uint256 counter = 0;

        for (uint256 i = 0; i < assets.length; i++) {
            if (assets[i].borrowAmount > 0 && assets[i].collateralAmount > 0) {
                tempResult[counter] = assets[i];
                counter++;
            }
        }

        assembly {
            mstore(tempResult, counter)
        }

        // Sorting logic: sort the array by the minimum of (borrowValue * 1.1) and collateralValue
        for (uint256 i = 0; i < counter; i++) {
            for (uint256 j = i + 1; j < counter; j++) {
                uint256 valueI = min(tempResult[i].borrowValue * 11 / 10, tempResult[i].collateralValue);
                uint256 valueJ = min(tempResult[j].borrowValue * 11 / 10, tempResult[j].collateralValue);

                // Swap elements if needed
                if (valueI < valueJ) {
                    AssetData memory temp = tempResult[i];
                    tempResult[i] = tempResult[j];
                    tempResult[j] = temp;
                }
            }
        }

        return tempResult;
    }

    function min(uint256 a, uint256 b) private pure returns (uint256) {
        return a < b ? a : b;
    }

    // Pick the borrow that is largest
    // Collateral of the same token if there is enough, if not then the one with most collateral
    function pickRepayAndCollateralAssets(AssetData[] memory assets)
        private
        pure
        returns (AssetData memory repayAsset, AssetData memory collateralAsset)
    {
        uint256 largestBorrowValue = 0;
        bool borrowAssetHasEnoughCollateral = false;

        uint256 largestCollateralValue = 0;
        AssetData memory largestCollateralAsset;

        for (uint256 i = 0; i < assets.length; i++) {
            AssetData memory asset = assets[i];

            uint256 collateralValue = assets[i].collateralAmount * asset.exchangeRate / 1e18;
            if (collateralValue > asset.cash) {
                collateralValue = asset.cash;
            }
            collateralValue = collateralValue * asset.price / 1e18;
            if (collateralValue > largestCollateralValue) {
                largestCollateralValue = collateralValue;
                largestCollateralAsset = asset;
            }

            uint256 borrowValue = asset.borrowAmount * asset.price / 1e18;
            if (borrowValue > largestBorrowValue) {
                largestBorrowValue = borrowValue;
                repayAsset = asset;

                // Check if the collateral is enough for the max liquidation of this token
                uint256 repayValue = borrowValue / 2; // 50% close factor
                uint256 redeemValue = repayValue * 11 / 10; // 10% incentive. TODO use the current percentage

                borrowAssetHasEnoughCollateral = collateralValue >= redeemValue;
            }
        }

        if (borrowAssetHasEnoughCollateral) {
            collateralAsset = repayAsset;
        } else {
            collateralAsset = largestCollateralAsset;
        }

        return (repayAsset, collateralAsset);
    }

    function getAssets(address borrower) private returns (AssetData[] memory) {
        uint256 snapshotId = vm.snapshot();
        address[] memory vtokens = comptroller.getAssetsIn(borrower);
        vm.revertTo(snapshotId);

        AssetData[] memory assets = new AssetData[](vtokens.length);
        uint256 validAssetCount = 0;

        for (uint256 i = 0; i < vtokens.length; i++) {
            (AssetData memory asset, bool success) = getAsset(borrower, VTokenInterface(vtokens[i]));

            if (success && (asset.collateralValue > 0 || asset.borrowAmount > 0)) {
                assets[validAssetCount] = asset;
                validAssetCount++;
            }
        }
        if (validAssetCount != vtokens.length) {
            AssetData[] memory newAssets = new AssetData[](validAssetCount);
            for (uint256 i = 0; i < validAssetCount; i++) {
                newAssets[i] = assets[i];
            }
            assets = newAssets;
        }
        return assets;
    }

    function getAsset(address borrower, VTokenInterface vtoken) private returns (AssetData memory, bool) {
        uint256 snapshotId = vm.snapshot();

        if (vtoken.accrualBlockNumber() > block.number) {
            console.log(
                "detected an invalid vtoken with accrualBlockNumber higher than the current block",
                vtoken.accrualBlockNumber(),
                block.number
            );
            AssetData memory noData;
            return (noData, false);
        }

        vtoken.accrueInterest();

        AssetData memory asset;
        asset.symbol = vtoken.symbol();
        asset.vtoken = vtoken;
        (, asset.collateralFactor) = comptroller.markets(address(vtoken));
        asset.collateralAmount = vtoken.balanceOf(borrower);
        asset.borrowAmount = vtoken.borrowBalanceCurrent(borrower);
        asset.cash = vtoken.getCash();
        asset.exchangeRate = vtoken.exchangeRateCurrent();
        asset.price = comptroller.oracle().getUnderlyingPrice(address(vtoken));
        asset.borrowValue = asset.borrowAmount * asset.price / 1e18;
        asset.collateralValue = asset.collateralAmount * asset.exchangeRate / 1e18;
        if (asset.collateralValue > asset.cash) {
            //revert("watch this");
            asset.isCollateralCappedByCash = true;
            asset.collateralValue = asset.cash;
        }
        asset.collateralValue = asset.collateralValue * asset.price / 1e18;

        vm.revertTo(snapshotId);
        return (asset, true);
    }

    function logAssets(AssetData[] memory assets) private pure {
        for (uint256 i = 0; i < assets.length; i++) {
            console.log("asset start", i);
            logAsset(assets[i]);
            console.log("asset end");
        }
    }

    function logAsset(AssetData memory asset) private pure {
        console.log("symbol", asset.symbol);
        console.log("vtoken", address(asset.vtoken));
        console.log("collateralFactor", asset.collateralFactor);
        console.log("collateralAmount", asset.collateralAmount);
        console.log("borrowAmount", asset.borrowAmount);
        console.log("cash", asset.cash);
        console.log("exchangeRate", asset.exchangeRate);
        console.log("price", asset.price);
        console.log("borrowValueUsd", ratioToString(asset.borrowValue, 1e18));
        console.log("collateralValueUsd", ratioToString(asset.collateralValue, 1e18));
        console.log("isCollateralCappedByCash", asset.isCollateralCappedByCash);
    }

    function getChainCoinPrice() private returns (uint256 price) {
        uint256 snapshot = vm.snapshot();
        price = comptroller.oracle().getUnderlyingPrice(address(0xA07c5b74C9B40447a954e1466938b865b6BBea36));
        vm.revertTo(snapshot);
        return price;
    }

    function maybeApproveRepayToken(
        VTokenInterface vToken,
        uint256 approveAmount,
        StrategyRunReport storage report
    )
        private
    {
        AssetReport storage asset = findAsset(report, vToken);
        if (compareStrings(asset.initialData.symbol, "vBNB")) {
            // Chain coin does not need approve
            return;
        }

        address approveAddress = address(vToken);
        address liquidator = getLiquidatorContract();
        if (liquidator != address(0)) {
            approveAddress = liquidator;
        }

        uint256 snapshot = vm.snapshot();
        EIP20Interface underlyingToken = EIP20Interface(vToken.underlying());
        vm.revertTo(snapshot);
        uint256 gasBefore = gasleft();
        underlyingToken.approve(approveAddress, approveAmount);
        uint256 gasAfter = gasleft();
        uint256 gasUsed = gasBefore - gasAfter - 10;

        asset.gasUsedToApprove += gasUsed;
    }

    function findAsset(
        StrategyRunReport storage report,
        VTokenInterface vtoken
    )
        private
        view
        returns (AssetReport storage)
    {
        for (uint256 i = 0; i < report.assets.length; i += 1) {
            if (report.assets[i].initialData.vtoken == vtoken) {
                return report.assets[i];
            }
        }
        revert("could not find asset in report");
    }

    struct FindRepayAmountVars {
        uint256 loopSnapshotId;
        uint256 low;
        uint256 high;
        uint256 mid;
        uint256 allowResult;
        uint256 minRepayAmount;
    }

    function findRepayAmountThatKeepsBorrowerLiquidatable(
        VTokenInterface repayVToken,
        address borrower,
        VTokenInterface collateralVToken,
        uint256 closeFactorRepayAmount
    )
        private
        returns (uint256)
    {
        FindRepayAmountVars memory vars;
        vars.loopSnapshotId = vm.snapshot();
        vars.low = 1;
        vars.high = closeFactorRepayAmount;
        vars.minRepayAmount = closeFactorRepayAmount; // Initialize with max value

        {
            // First, check the maximum repayAmount (closeFactorRepayAmount)
            callLiquidate(borrower, repayVToken, collateralVToken, closeFactorRepayAmount);

            (vars.allowResult,,) = callLiquidateWithResult(borrower, repayVToken, collateralVToken, 1);

            vm.revertTo(vars.loopSnapshotId);

            if (vars.allowResult == 0) {
                console.log("Max repayAmount will keep the borrower liquidatable!!!");
                return closeFactorRepayAmount;
            }
        }

        while (vars.low <= vars.high) {
            vars.mid = (vars.low + vars.high) / 2;

            callLiquidate(borrower, repayVToken, collateralVToken, vars.mid);

            // Want to check if later we are able to repay at the very least closeFactorRepayAmount-repayAmount
            (vars.allowResult,,) =
                callLiquidateWithResult(borrower, repayVToken, collateralVToken, closeFactorRepayAmount - vars.mid);

            // console.log("Trying repayAmount:", vars.mid, "allowResult:", vars.allowResult);

            if (vars.allowResult > 0) {
                vars.minRepayAmount = vars.mid;
                vars.high = vars.mid - 1;
            } else {
                vars.low = vars.mid + 1;
            }

            vm.revertTo(vars.loopSnapshotId);
        }

        uint256 maxRepayAmount = vars.minRepayAmount - 1;
        // console.log("Maximum repayAmount that keeps borrower liquidatable", maxRepayAmount);
        vm.revertTo(vars.loopSnapshotId);
        return maxRepayAmount;
    }

    function findRepayAmountThatKeepsBorrowerLiquidatableWithMultiCollateralSupport(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        VTokenInterface otherCollateralVToken,
        uint256 otherCollateralRepayAmountBase,
        uint256 freeRepayAmount
    )
        private
        returns (uint256)
    {
        (uint256 maxRepayAmount,) = findMaxRepayAmount(repayVToken, collateralVToken, borrower);
        if (maxRepayAmount == 0) return 0;

        FindRepayAmountVars memory vars;
        vars.loopSnapshotId = vm.snapshot();
        vars.low = 1;
        vars.high = maxRepayAmount;
        vars.minRepayAmount = maxRepayAmount; // Initialize with max value

        {
            // First, check the maximum repayAmount
            callLiquidate(borrower, repayVToken, collateralVToken, maxRepayAmount);

            uint256 otherRepayAmount;
            if (maxRepayAmount <= freeRepayAmount) otherRepayAmount = otherCollateralRepayAmountBase;
            else otherRepayAmount = otherCollateralRepayAmountBase - (maxRepayAmount - freeRepayAmount + 1) / 2;
            (vars.allowResult,,) =
                callLiquidateWithResult(borrower, repayVToken, otherCollateralVToken, otherRepayAmount);

            vm.revertTo(vars.loopSnapshotId);

            if (vars.allowResult == 0) {
                console.log("Max repayAmount will keep the borrower liquidatable!!!");
                return maxRepayAmount;
            }
        }

        while (vars.low <= vars.high) {
            vars.mid = (vars.low + vars.high) / 2;

            callLiquidate(borrower, repayVToken, collateralVToken, vars.mid);

            // Want to check if later we are able to repay at the very least otherRepayAmount
            uint256 otherRepayAmount;
            if (vars.mid <= freeRepayAmount) otherRepayAmount = otherCollateralRepayAmountBase;
            else otherRepayAmount = otherCollateralRepayAmountBase - (vars.mid - freeRepayAmount + 1) / 2;
            (vars.allowResult,,) =
                callLiquidateWithResult(borrower, repayVToken, otherCollateralVToken, otherRepayAmount);

            if (vars.allowResult > 0) {
                vars.minRepayAmount = vars.mid;
                vars.high = vars.mid - 1;
            } else {
                vars.low = vars.mid + 1;
            }

            vm.revertTo(vars.loopSnapshotId);
        }

        uint256 repayAmountThatKeepsBorrowerLiquidatable = vars.minRepayAmount - 1;
        vm.revertTo(vars.loopSnapshotId);
        return repayAmountThatKeepsBorrowerLiquidatable;
    }

    function findBorrowBalance(VTokenInterface repayVToken, address borrower) private returns (uint256 borrowBalance) {
        uint256 snapshotId = vm.snapshot();
        borrowBalance = VTokenInterface(repayVToken).borrowBalanceCurrent(borrower);
        vm.revertTo(snapshotId);
    }

    function findCloseFactorRepayAmount(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken
    )
        private
        returns (uint256 repayAmount)
    {
        uint256 snapshot = vm.snapshot();
        uint256 borrowBalance = VTokenInterface(repayVToken).borrowBalanceCurrent(borrower);
        repayAmount = borrowBalance / 2; // Take 50%
        if (isVTokenChainCoin(repayVToken)) {
            // Repay amount increases the cash before the liquidation happens
            // In turn it increases the exchange rate which means the borrowwers borrow balance increases to our benefit
            // But if the borrower has a lot of collateral the increased exchange rate could make the borrower healty.
            // If the borrower has significantly more borrow than collateral then the maximum repay amount by close
            // factor is greater than the current borrow balance /2.

            // We assume that the maximum repay amount lies between [0, borrowBalance]
            // If not then we will later revert.
            uint256 left = 0;
            uint256 right = borrowBalance;

            if (right == 0) {
                vm.revertTo(snapshot);
                return 0;
            }

            while (left <= right) {
                uint256 mid = (left + right) / 2;

                (uint256 errorCode,,) = callLiquidateWithResult(borrower, repayVToken, collateralVToken, mid);
                if (errorCode == 0) left = mid + 1;
                else right = mid - 1;
                vm.revertTo(snapshot);
            }
            repayAmount = left - 1;
        }
        vm.revertTo(snapshot);

        {
            // Try to liquidate with 1 wei more - it should fail
            snapshot = vm.snapshot();
            callLiquidateExpectFail(borrower, repayVToken, collateralVToken, repayAmount + 1);
            vm.revertTo(snapshot);
        }
    }

    function findCollateralCappedRepayAmount(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken
    )
        private
        returns (uint256 repayAmount)
    {
        Exp memory ratio = findBorrowToCollateralRatio(repayVToken, collateralVToken);

        uint256 snapshotId = vm.snapshot();
        uint256 collateralBalance = collateralVToken.balanceOf(borrower);
        vm.revertTo(snapshotId);

        bool testLiquidationWithPlusOne = true;
        {
            // Respect cash
            // TODO: ideally reiktu ivertinti kiek jau iki siol planuojam cash'outinti
            uint256 snapshot = vm.snapshot();
            uint256 cash = collateralVToken.getCash();
            uint256 exchangeRate = collateralVToken.exchangeRateCurrent();
            vm.revertTo(snapshot);

            // Includes overpay
            uint256 redeemableCollateral = div_roundingUp(cash + 1, Exp({ mantissa: exchangeRate })) - 1;
            if (redeemableCollateral < collateralBalance) {
                collateralBalance = redeemableCollateral;
                testLiquidationWithPlusOne = false;

                {
                    // Try to redeem with 1 wei more - it should fail because not enough cash
                    snapshot = vm.snapshot();
                    setTokenBalance(address(collateralVToken), redeemableCollateral + 1);
                    redeemCollateralExpectFail(collateralVToken, redeemableCollateral + 1);
                    vm.revertTo(snapshot);
                }
            }
        }

        repayAmount = div_roundingUp(collateralBalance + 1, ratio) - 1; // Includes overpay

        if (testLiquidationWithPlusOne) {
            // Try to liquidate with 1 wei more - it should fail
            uint256 snapshot = vm.snapshot();
            callLiquidateExpectFail(borrower, repayVToken, collateralVToken, repayAmount + 1);
            vm.revertTo(snapshot);
        }
    }

    function findSmallestEffectiveRepayAmount(
        VTokenInterface vTokenBorrowed,
        VTokenInterface vTokenCollateral,
        uint256 repayAmount,
        address borrower // For testing to ensure the correctness of the result
    )
        private
        returns (uint256)
    {
        if (repayAmount == 0) return 0;

        // Ensure that the effective repay amount gives the same collateral as the original repay amount
        uint256 collateralVTokenGainedWithOriginalRepayAmount;
        {
            uint256 snapshot = vm.snapshot();
            (collateralVTokenGainedWithOriginalRepayAmount,) =
                callLiquidate(borrower, vTokenBorrowed, vTokenCollateral, repayAmount);
            vm.revertTo(snapshot);
        }

        Exp memory ratio = findBorrowToCollateralRatio(vTokenBorrowed, vTokenCollateral);

        // Floor the repay amount to the effective amount, where decreasing any more would result into smaller seize
        // repayAmount * ratio / ratio (division rounding up)
        uint256 effectiveRepayAmount = div_roundingUp(mul_ScalarTruncate(ratio, repayAmount), ratio);

        if (getLiquidatorContract() != address(0) && effectiveRepayAmount > 0) {
            uint256 seize = mul_ScalarTruncate(ratio, effectiveRepayAmount);
            if (seize % 22 == 0) {
                seize -= 1;
            }
            effectiveRepayAmount = div_roundingUp(seize, ratio);
        }

        // Ensure that decreasing the repay amount anything below effective value would result in less collateral seized
        uint256 collateralVTokenGainedWithOptimalAmountInMinus;
        if (effectiveRepayAmount > 1) {
            uint256 snapshot = vm.snapshot();
            (collateralVTokenGainedWithOptimalAmountInMinus,) =
                callLiquidate(borrower, vTokenBorrowed, vTokenCollateral, effectiveRepayAmount - 1);
            vm.revertTo(snapshot);
        }

        uint256 collateralVTokenGainedWithOptimalAmountIn;
        if (effectiveRepayAmount > 0) {
            uint256 snapshot = vm.snapshot();
            (collateralVTokenGainedWithOptimalAmountIn,) =
                callLiquidate(borrower, vTokenBorrowed, vTokenCollateral, effectiveRepayAmount);
            vm.revertTo(snapshot);
        }

        assertEq(
            collateralVTokenGainedWithOptimalAmountIn,
            collateralVTokenGainedWithOriginalRepayAmount,
            "Original repay amount should give the same collateral as effective repay amount"
        );
        if (effectiveRepayAmount > 0) {
            assertGt(
                collateralVTokenGainedWithOptimalAmountIn,
                collateralVTokenGainedWithOptimalAmountInMinus,
                "One less the effective repay amount should give less collateral"
            );
        }

        return effectiveRepayAmount;
    }

    function findBorrowToCollateralRatio(
        VTokenInterface vTokenBorrowed,
        VTokenInterface vTokenCollateral
    )
        private
        returns (Exp memory ratio)
    {
        uint256 snapshotId = vm.snapshot();
        uint256 priceBorrowedMantissa = comptroller.oracle().getUnderlyingPrice(address(vTokenBorrowed));
        uint256 priceCollateralMantissa = comptroller.oracle().getUnderlyingPrice(address(vTokenCollateral));
        require(priceBorrowedMantissa != 0 && priceCollateralMantissa != 0);

        Exp memory numerator = mul_(
            Exp({ mantissa: ComptrollerInterface(comptroller).liquidationIncentiveMantissa() }),
            Exp({ mantissa: priceBorrowedMantissa })
        );
        Exp memory denominator =
            mul_(Exp({ mantissa: priceCollateralMantissa }), Exp({ mantissa: vTokenCollateral.exchangeRateCurrent() }));
        ratio = div_(numerator, denominator);
        vm.revertTo(snapshotId);
    }

    function findMaxRepayAmount(
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        address borrower
    )
        private
        returns (uint256 repayAmount, bool isCappedByCollateral)
    {
        uint256 snapshot = vm.snapshot();
        (uint256 errorCode,,) = callLiquidateWithResult(borrower, repayVToken, collateralVToken, 1);
        vm.revertTo(snapshot);
        if (errorCode != 0) return (repayAmount, isCappedByCollateral);

        uint256 cappedByCloseFactor = findCloseFactorRepayAmount(borrower, repayVToken, collateralVToken);
        uint256 cappedByCollateral = findCollateralCappedRepayAmount(borrower, repayVToken, collateralVToken);

        if (cappedByCollateral < cappedByCloseFactor) {
            repayAmount = cappedByCollateral;
            isCappedByCollateral = true;
        } else {
            repayAmount = cappedByCloseFactor;
        }
    }

    function assertAssumtions(VTokenInterface collateralVToken) private {
        uint256 snapshot = vm.snapshot();
        uint256 incentive = comptroller.liquidationIncentiveMantissa();
        assertEq(incentive, 1_100_000_000_000_000_000, "Liquidation incentice is not 10%");
        console.log("incentive", incentive);

        uint256 closeFactor = comptroller.closeFactorMantissa();
        assertEq(closeFactor, 500_000_000_000_000_000, "Close factor is not 50%");

        address liquidator = getLiquidatorContract();
        if (liquidator != address(0)) {
            uint256 treasuryPercentMantissa = Liquidator(liquidator).treasuryPercentMantissa();
            assertEq(treasuryPercentMantissa, 50_000_000_000_000_000, "Liquidator treasury percent is not 5%");
        }

        assertWeDontHaveVCollateral(collateralVToken);

        vm.revertTo(snapshot);
    }

    function assertWeDontHaveVCollateral(VTokenInterface collateralVToken) private view {
        assertEq(collateralVToken.balanceOf(myAddress), 0);
    }

    function dealUnderlyingTokens(VTokenInterface vToken, uint256 amount) private {
        if (isVTokenChainCoin(vToken)) {
            vm.deal(myAddress, amount);
        } else {
            EIP20Interface underlyingToken = EIP20Interface(vToken.underlying());
            setTokenBalance(address(underlyingToken), amount);
        }
    }

    function getLiquidatorContract() private returns (address liquidator) {
        uint256 snapshot = vm.snapshot();
        try comptroller.liquidatorContract() returns (address liquidatorResult) {
            liquidator = liquidatorResult;
        } catch { }
        vm.revertTo(snapshot);
    }

    function isVTokenChainCoin(VTokenInterface vtoken) private returns (bool) {
        uint256 snapshot = vm.snapshot();
        bool isChainCoin = compareStrings(vtoken.symbol(), "vBNB");
        vm.revertTo(snapshot);
        return isChainCoin;
    }

    struct CallLiquidateIfCoversGasVars {
        uint256 snapshot;
        uint256 collateralVTokenGained;
        uint256 gasUsed;
    }

    function callLiquidateIfCoversGas(
        address borrower,
        AssetData memory repayAsset,
        AssetData memory collateralAsset,
        uint256 repayAmount,
        StrategyRunReport storage report
    )
        private
        returns (bool profitCoversGas)
    {
        CallLiquidateIfCoversGasVars memory vars;
        vars.snapshot = vm.snapshot();
        (vars.collateralVTokenGained, vars.gasUsed) =
            callLiquidate(borrower, repayAsset.vtoken, collateralAsset.vtoken, repayAmount);

        uint256 collateralUnderlyingGained = simulateRedeem(collateralAsset.vtoken, vars.collateralVTokenGained);
        uint256 repayUsd = repayAmount * repayAsset.price / 1e18;
        uint256 seizedUsd = collateralUnderlyingGained * collateralAsset.price / 1e18;
        uint256 gasInUsd = report.gasPrice * vars.gasUsed * report.chainCoinPrice / 1e18;
        console.log(
            "did a liquidation (repayUsd, seizedUsd, gasInUsd)",
            ratioToString(repayUsd, 1e18),
            ratioToString(seizedUsd, 1e18),
            ratioToString(gasInUsd, 1e18)
        );

        profitCoversGas = seizedUsd - repayUsd > gasInUsd;
        if (profitCoversGas) {
            report.liquidations.push(
                LiquidationReport({
                    repaySymbol: repayAsset.symbol,
                    repayVToken: repayAsset.vtoken,
                    collateralSymbol: collateralAsset.symbol,
                    collateralVToken: collateralAsset.vtoken,
                    repayAmount: repayAmount,
                    collateralVTokenGained: vars.collateralVTokenGained,
                    collateralUnderlyingGained: collateralUnderlyingGained,
                    gasUsed: vars.gasUsed,
                    postHealthFactor: getHealthFactor(borrower),
                    repaidUsd: repayUsd,
                    seizedUsd: seizedUsd
                })
            );
        } else {
            vm.revertTo(vars.snapshot);
            console.log(
                "liquidation does not cover gas cost (revenue, cost)",
                ratioToString(seizedUsd - repayUsd, 1e18),
                ratioToString(gasInUsd, 1e18)
            );
        }
    }

    function callLiquidate(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        uint256 repayAmount
    )
        private
        returns (uint256 collateralVTokenGained, uint256 gasUsed)
    {
        uint256 errorCode;
        (errorCode, collateralVTokenGained, gasUsed) =
            callLiquidateWithResult(borrower, repayVToken, collateralVToken, repayAmount);
        assertEq(errorCode, 0, "Liquidation failed");
    }

    function callLiquidateExpectFail(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        uint256 repayAmount
    )
        private
    {
        (uint256 errorCode,,) = callLiquidateWithResult(borrower, repayVToken, collateralVToken, repayAmount);
        require(errorCode != 0, "Expected the liquidation to fail");
    }

    function callLiquidateWithResult(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        uint256 repayAmount
    )
        private
        returns (uint256 errorCode, uint256 collateralVTokenGained, uint256 gasUsed)
    {
        bool isRepayChainCoin = isVTokenChainCoin(repayVToken);
        collateralVTokenGained = collateralVToken.balanceOf(myAddress);
        address liquidator = getLiquidatorContract();
        if (liquidator == address(0)) {
            VBep20Interface repayVBep20 = VBep20Interface(address(repayVToken));

            uint256 gasBefore = gasleft();
            if (isRepayChainCoin) {
                try repayVBep20.liquidateBorrow{ value: repayAmount }(borrower, collateralVToken) { }
                catch {
                    errorCode = 1;
                }
            } else {
                // Sometimes fails without a revert
                try repayVBep20.liquidateBorrow(borrower, repayAmount, collateralVToken) returns (uint256 result) {
                    errorCode = result;
                } catch {
                    errorCode = 1;
                }
            }
            uint256 gasAfter = gasleft();
            gasUsed = gasBefore - gasAfter - 10;
        } else {
            uint256 nativeAmount;
            if (isRepayChainCoin) {
                nativeAmount = repayAmount;
            }
            uint256 gasBefore = gasleft();
            try Liquidator(liquidator).liquidateBorrow{ value: nativeAmount }(
                address(repayVToken), borrower, repayAmount, address(collateralVToken)
            ) { } catch {
                errorCode = 1;
            }
            uint256 gasAfter = gasleft();
            gasUsed = gasBefore - gasAfter - 10;
        }
        collateralVTokenGained = collateralVToken.balanceOf(myAddress) - collateralVTokenGained;
    }

    function redeemCollateralExpectFail(VTokenInterface vToken, uint256 amount) private {
        uint256 success = VBep20Interface(address(vToken)).redeem(amount);
        assertTrue(success != 0, "Expected the redeem to fail");
    }

    function redeemCollateral(
        VTokenInterface vToken,
        uint256 amount
    )
        private
        returns (uint256 underlyingGained, uint256 gasUsed)
    {
        uint256 exchangeRate;
        uint256 cash;
        {
            uint256 snapshotId = vm.snapshot();
            exchangeRate = vToken.exchangeRateCurrent();
            cash = vToken.getCash();
            vm.revertTo(snapshotId);
        }

        uint256 predictedUnderlying = amount * exchangeRate / 1e18;
        bool enoughCash = cash >= predictedUnderlying;

        if (!enoughCash) {
            revert("not enough cash to redeem");
            // TODO: gali buti reikes nuimti revert nes kartais apsimokes gauti daugiau vtoken nei yra cash
            amount = cash * 1e18 / exchangeRate;
        }

        uint256 predictedUnderlyingWithoutRedeemFee = predictedUnderlying;
        {
            uint256 treasuryPercent;
            try comptroller.treasuryPercent() returns (uint256 treasuryPercentResult) {
                treasuryPercent = treasuryPercentResult;
            } catch {
                // The function could not exist
            }
            uint256 feeAmount = predictedUnderlying * treasuryPercent / 1e18;
            predictedUnderlying -= feeAmount;
        }

        {
            uint256 collateralBalanceBefore;
            bool isCollateralChainCoin = isVTokenChainCoin(vToken);
            if (isCollateralChainCoin) collateralBalanceBefore = myAddress.balance;
            else collateralBalanceBefore = EIP20Interface(vToken.underlying()).balanceOf(myAddress);
            uint256 redeemGasBefore = gasleft();
            uint256 success = VBep20Interface(address(vToken)).redeem(amount);
            uint256 redeemGasAfter = gasleft();
            assertEq(success, 0, "Redeem failed");
            gasUsed = redeemGasBefore - redeemGasAfter - 10;
            if (isCollateralChainCoin) underlyingGained = myAddress.balance - collateralBalanceBefore;
            else underlyingGained = EIP20Interface(vToken.underlying()).balanceOf(myAddress) - collateralBalanceBefore;
        }

        if (predictedUnderlying != underlyingGained) {
            console.log("predictedUnderlying", predictedUnderlying);
            console.log("predictedUnderlyingWithoutRedeemFee", predictedUnderlyingWithoutRedeemFee);
            console.log("underlyingGained", underlyingGained);

            if (predictedUnderlyingWithoutRedeemFee == underlyingGained) {
                console.log("treasuryPercent configured, but not applied for this token", address(vToken));
            } else {
                revert("underlyingGained does not match predictedUnderlying or predictedUnderlyingWithoutRedeemFee");
            }
        }

        return (underlyingGained, gasUsed);
    }

    function setTokenBalance(address token, uint256 balance) private {
        stdstore.target(token).sig(EIP20Interface.balanceOf.selector).with_key(myAddress).checked_write(balance);
        uint256 storedBalance = EIP20Interface(token).balanceOf(myAddress);
        assertEq(storedBalance, balance, "My fake balance is not correct");
    }

    function compareStrings(string memory a, string memory b) internal pure returns (bool) {
        return (keccak256(abi.encodePacked((a))) == keccak256(abi.encodePacked((b))));
    }
}
