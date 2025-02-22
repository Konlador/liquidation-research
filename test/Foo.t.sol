// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.6.2;
pragma experimental ABIEncoderV2;

import { Test } from "forge-std/src/Test.sol";
import "forge-std/src/Test.sol";
import { console } from "forge-std/src/console.sol";

import { ComptrollerInterface } from "../src/Comptroller/ComptrollerInterface.sol";
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

    function setUp() public virtual { }

    function testLiquidations() external {
        bytes32 txHashLiquidation = vm.envBytes32("TX_HASH");
        VTokenInterface repayVToken = VTokenInterface(vm.envAddress("REPAY_V_TOKEN"));
        address borrower = vm.envAddress("BORROWER");
        uint256 repayAmount = vm.envUint("REPAY_AMOUNT");
        VTokenInterface collateralVToken = VTokenInterface(vm.envAddress("COLLATERAL_V_TOKEN"));
        uint256 expectedSeize = vm.envUint("EXPECTED_SEIZE");

        uint256 txFork = vm.createSelectFork(QUICKNODE_RPC_URL, txHashLiquidation);
        vm.createSelectFork(QUICKNODE_RPC_URL, block.number + 1);
        uint256 nextBlockTime = block.timestamp; // Sometimes the time diff is not 3s (30935497-30935498 4s)
        vm.selectFork(txFork);

        // Important, because the default is the previous block metadata
        vm.roll(block.number + 1);
        vm.warp(nextBlockTime);
        // assertTrue(block.number < 31_302_048, "not in istanbul");
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
                expectedSeize -= expectedSeize / 22; // (seizedAmount * treasuryPercentMantissa) / totalIncentive;
            }
        }

        // // #1 Repeat
        // repeatLiquidation(repayVToken, borrower, repayAmount, collateralVToken, expectedSeize);
        // vm.revertTo(freshSnapshotId);

        // // #2 Up to close factor
        // upToCloseFactorLiquidation(repayVToken, borrower, collateralVToken, expectedSeize);
        // vm.revertTo(freshSnapshotId);

        // // #3 Drain
        // drainLiquidation(repayVToken, borrower, collateralVToken, expectedSeize);
        // vm.revertTo(freshSnapshotId);

        // #4 Largest pair
        largestPair(borrower);
        vm.revertTo(freshSnapshotId);

        // // #5 From smallest collateral factor
        // fromSmallestCollateralFactor(borrower);
        // vm.revertTo(freshSnapshotId);

        // // #6 From largest collateral factor
        // fromLargestCollateralFactor(borrower);
        // vm.revertTo(freshSnapshotId);
    }

    function repeatLiquidation(
        VTokenInterface repayVToken,
        address borrower,
        uint256 repayAmount,
        VTokenInterface collateralVToken,
        uint256 expectedSeize
    )
        private
    {
        console.log("");
        console.log("Tests case: repeatLiquidation");
        assertAssumtions(collateralVToken);

        bool isRepayChainCoin = dealUnderlyingTokens(repayVToken, repayAmount + 1);
        maybeApproveRepayTokenAndLog(isRepayChainCoin, repayVToken, repayAmount);

        console.log("repayAmount", repayAmount);
        (uint256 collateralVTokenGained, uint256 gasUsed) =
            callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);

        console.log("collateralVTokenGained", collateralVTokenGained);
        console.log("gasUsedForLiquidation", gasUsed);
        assertEq(collateralVTokenGained, expectedSeize, "expected seize does not match");

        redeemCollateral(collateralVToken, collateralVTokenGained);
        logOraclePrices(repayVToken, collateralVToken);

        console.log("Tests case end");
    }

    function upToCloseFactorLiquidation(
        VTokenInterface repayVToken,
        address borrower,
        VTokenInterface collateralVToken,
        uint256 originalSeizeAmount
    )
        private
    {
        console.log("");
        console.log("Tests case: upToCloseFactorLiquidation");

        assertAssumtions(collateralVToken);

        (uint256 repayAmount, bool cappedByCollateral) = findMaxRepayAmount(repayVToken, collateralVToken, borrower);
        bool isRepayChainCoin = dealUnderlyingTokens(repayVToken, repayAmount + 1);
        maybeApproveRepayTokenAndLog(isRepayChainCoin, repayVToken, repayAmount);

        console.log("maxRepayAmount", repayAmount);

        repayAmount = adjustRepayAmount(repayVToken, collateralVToken, repayAmount, borrower);

        if (repayAmount == 0) {
            console.log("No collateral to be gained!!!");
        }

        console.log("repayAmount", repayAmount);
        console.log("cappedByCollateral", cappedByCollateral);
        uint256 collateralVTokenGained;
        uint256 gasUsed;
        if (repayAmount > 0) {
            (collateralVTokenGained, gasUsed) =
                callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
        }

        console.log("collateralVTokenGained", collateralVTokenGained);
        console.log("gasUsedForLiquidation", gasUsed);
        assertGe(
            collateralVTokenGained,
            originalSeizeAmount,
            "Expected to get greater or equal collateral than the original liquidation"
        );

        redeemCollateral(collateralVToken, collateralVTokenGained);
        logOraclePrices(repayVToken, collateralVToken);

        console.log("Tests case end");
    }

    function drainLiquidation(
        VTokenInterface repayVToken,
        address borrower,
        VTokenInterface collateralVToken,
        uint256 originalSeizeAmount
    )
        private
    {
        console.log("");
        console.log("Tests case: drainLiquidation");
        assertAssumtions(collateralVToken);

        uint256 borrowBalance = findBorrowBalance(repayVToken, borrower);
        bool isRepayChainCoin = dealUnderlyingTokens(repayVToken, borrowBalance + 1);
        maybeApproveRepayTokenAndLog(isRepayChainCoin, repayVToken, borrowBalance);

        uint256 repayAmountTotal = 0;
        uint256 liquidationCount = 1;
        while (true) {
            (uint256 maxRepayAmount, bool cappedByCollateral) =
                findMaxRepayAmount(repayVToken, collateralVToken, borrower);
            if (maxRepayAmount <= 1 || cappedByCollateral) break;

            uint256 repayAmountThatKeepsBorrowerLiquidatable = findRepayAmountThatKeepsBorrowerLiquidatable(
                repayVToken, borrower, collateralVToken, isRepayChainCoin, maxRepayAmount
            );
            // console.log("repayAmountThatKeepsBorrowerLiquidatable", repayAmountThatKeepsBorrowerLiquidatable);

            uint256 repayAmount =
                adjustRepayAmount(repayVToken, collateralVToken, repayAmountThatKeepsBorrowerLiquidatable, borrower);
            if (repayAmount == 0) {
                break;
            }

            console.log(liquidationCount, "repayAmount", repayAmount);
            repayAmountTotal += repayAmount;
            (uint256 collateralVTokenGained, uint256 gasUsed) =
                callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
            console.log(liquidationCount, "collateralVTokenGained", collateralVTokenGained);
            console.log(liquidationCount, "gasUsedForLiquidation", gasUsed);
            liquidationCount++;
            if (collateralVTokenGained == 0) {
                // Should never happen after adjustRepayAmount was implemented
                revert("collateral gained cannot be zero");
            }
            if (repayAmountThatKeepsBorrowerLiquidatable < maxRepayAmount) {
                break;
            }
        }
    }

    struct AssetData {
        VTokenInterface vtoken;
        uint256 collateralFactor;
        uint256 collateralAmount;
        uint256 borrowAmount;
        uint256 cash;
        uint256 exchangeRate;
        uint256 price;
    }

    // Ok, svarbu galeti palyginti su paprastais case'ais
    // Norisi tureti viena kur padarom tik L_max likvidacija ir viskas
    // Taip pat noretusi tureti case, kur likviduojame pradedant nuo didziausio collateralFactor
    function largestPair(address borrower) private {
        console.log("");
        console.log("Tests case: largestPair");
        // assertAssumtions(collateralVToken);

        // VTokenInterface repayVToken,
        // VTokenInterface collateralVToken,

        AssetData[] memory assets = getAssets(borrower);
        logAssets(assets);

        (AssetData memory largestBorrow, AssetData memory largestCollateral) = getLargestAssets(assets);
        console.log("largestBorrow", address(largestBorrow.vtoken));
        console.log("largestCollateral", address(largestCollateral.vtoken));

        // jeigu taip gaunasi, ai nu tai nedarysim begalybe grazinamu vis mazindavmi return amount dvigubai

        // now find the max possible

        // pasirasom lestuka kuris patikrina kiekviena cap

        // L_max = (borrow token, collateral token, amount returned) - su uztektinu cash
        // Limit_cash = cash / collateralPrice * borrowPrice /* 1.05 (write a test for it)
        // L_max'e collateral ir borrow tokenai yra maksimaliai dideli.
        // likvidacija nedaro itakos exchange rate'ui, nes viska ka grazini prisideda prie cash.

        // Ieskome didziausio L_interim, kad butu galima likviduoti po to bent L_max.amountValue - L_interim.amountValue
        // Jeigu nebus galima, tai nera prasmes ji daryti ir darome paskutini.

        // Ok tai imam collateral tokena su maziausiu collateral factor
        // Tada pasirenkam maziausia skola, bandome likviduoti maksimaliai tiek
        // Kad po lividacijos bus galima likviduoti L_max - L_interim ir be to atsiimti visa cash kuris planuotas
        // atsiimti

        // Redeems padarysim paciame gale, kad taupyti gas ir bundle'inti

        // uint256 borrowBalance = findBorrowBalance(repayVToken, borrower);
        // bool isRepayChainCoin = dealUnderlyingTokens(repayVToken, borrowBalance + 1);
        // maybeApproveRepayTokenAndLog(isRepayChainCoin, repayVToken, borrowBalance);

        // uint256 repayAmountTotal = 0;
        // uint256 liquidationCount = 1;
        // while (true) {
        //     (uint256 maxRepayAmount, bool cappedByCollateral) =
        //         findMaxRepayAmount(repayVToken, collateralVToken, borrower);
        //     if (maxRepayAmount <= 1 || cappedByCollateral) break;

        //     uint256 repayAmountThatKeepsBorrowerLiquidatable = findRepayAmountThatKeepsBorrowerLiquidatable(
        //         repayVToken, borrower, collateralVToken, isRepayChainCoin, maxRepayAmount
        //     );
        //     // console.log("repayAmountThatKeepsBorrowerLiquidatable", repayAmountThatKeepsBorrowerLiquidatable);

        //     uint256 repayAmount =
        //         adjustRepayAmount(repayVToken, collateralVToken, repayAmountThatKeepsBorrowerLiquidatable, borrower);
        //     if (repayAmount == 0) {
        //         break;
        //     }

        //     console.log(liquidationCount, "repayAmount", repayAmount);
        //     repayAmountTotal += repayAmount;
        //     (uint256 collateralVTokenGained, uint256 gasUsed) =
        //         callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
        //     console.log(liquidationCount, "collateralVTokenGained", collateralVTokenGained);
        //     console.log(liquidationCount, "gasUsedForLiquidation", gasUsed);
        //     liquidationCount++;
        //     if (collateralVTokenGained == 0) {
        //         // Should never happen after adjustRepayAmount was implemented
        //         revert("collateral gained cannot be zero");
        //     }
        //     if (repayAmountThatKeepsBorrowerLiquidatable < maxRepayAmount) {
        //         break;
        //     }
        // }

        // // console.log(liquidationCount, "borrowersCollateralBalance", collateralVToken.balanceOf(borrower));

        // // Last liquidation
        // (uint256 repayAmount, bool cappedByCollateral) = findMaxRepayAmount(repayVToken, collateralVToken, borrower);

        // repayAmount = adjustRepayAmount(repayVToken, collateralVToken, repayAmount, borrower);
        // console.log("Last liquidation needed", repayAmount > 0);
        // if (repayAmount > 0) {
        //     console.log(liquidationCount, "repayAmount", repayAmount);
        //     repayAmountTotal += repayAmount;
        //     (uint256 collateralVTokenGained, uint256 gasUsed) =
        //         callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
        //     console.log(liquidationCount, "collateralVTokenGained", collateralVTokenGained);
        //     console.log(liquidationCount, "gasUsedForLiquidation", gasUsed);
        // }

        // console.log("cappedByCollateral", cappedByCollateral);

        // console.log("repayAmountTotal", repayAmountTotal);
        // uint256 collateralVTokenGainedTotal = collateralVToken.balanceOf(myAddress);
        // console.log("collateralVTokenGainedTotal", collateralVTokenGainedTotal);
        // assertGe(collateralVTokenGainedTotal, originalSeizeAmount);

        // redeemCollateral(collateralVToken, collateralVTokenGainedTotal);
        // logOraclePrices(repayVToken, collateralVToken);

        console.log("Tests case end");
    }

    function getLargestAssets(AssetData[] memory assets)
        private
        pure
        returns (AssetData memory largestBorrow, AssetData memory largestCollateral)
    {
        uint256 largestBorrowValue = 0;
        uint256 largestCollateralValue = 0;

        for (uint256 i = 0; i < assets.length; i++) {
            AssetData memory asset = assets[i];

            uint256 borrowValue = assets[i].borrowAmount * 1e18 / asset.price;
            if (borrowValue > largestBorrowValue) {
                largestBorrowValue = borrowValue;
                largestBorrow = asset;
            }

            uint256 collateralValue = assets[i].collateralAmount * asset.exchangeRate / asset.price;
            uint256 cashValue = asset.cash * 1e18 / asset.price;
            if (cashValue < collateralValue) {
                // If we can't cash out then the collateral is not as good.
                // Maybe calculate the effective collateral before and reuse here.
                collateralValue = cashValue;
            }
            // console.log("collateralValue", collateralValue);
            // console.log("cashValue      ", cashValue);
            if (collateralValue > largestCollateralValue) {
                largestCollateralValue = collateralValue;
                largestCollateral = asset;
            }
        }

        return (largestBorrow, largestCollateral);
    }

    function getAssets(address borrower) private returns (AssetData[] memory) {
        // take a snapshot
        address[] memory vtokens = comptroller.getAssetsIn(borrower);
        AssetData[] memory assets = new AssetData[](vtokens.length);
        uint256 validAssetCount = 0;

        PriceOracle oracle = comptroller.oracle();
        for (uint256 i = 0; i < vtokens.length; i++) {
            VTokenInterface vtoken = VTokenInterface(vtokens[i]);
            vtoken.accrueInterest();

            AssetData memory asset;
            asset.vtoken = vtoken;
            (, asset.collateralFactor) = comptroller.markets(address(vtoken));
            asset.collateralAmount = vtoken.balanceOf(borrower);
            asset.borrowAmount = vtoken.borrowBalanceCurrent(borrower);
            asset.cash = vtoken.getCash();
            asset.exchangeRate = vtoken.exchangeRateCurrent();
            asset.price = oracle.getUnderlyingPrice(address(vtoken));

            if (asset.collateralAmount > 0 || asset.borrowAmount > 0) {
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

    function logAssets(AssetData[] memory assets) private pure {
        for (uint256 i = 0; i < assets.length; i++) {
            console.log("assetIndex", i);
            console.log("vtoken", address(assets[i].vtoken));
            console.log("collateralFactor", assets[i].collateralFactor);
            console.log("collateralAmount", assets[i].collateralAmount);
            console.log("borrowAmount", assets[i].borrowAmount);
            console.log("cash", assets[i].cash);
            console.log("exchangeRate", assets[i].exchangeRate);
            console.log("price", assets[i].price);
            console.log("");
        }
    }

    function logOraclePrices(VTokenInterface repayVToken, VTokenInterface collateralVToken) private view {
        PriceOracle oracle = comptroller.oracle();
        uint256 repayTokenPrice = oracle.getUnderlyingPrice(address(repayVToken));
        console.log("repayTokenPrice", repayTokenPrice);
        uint256 collateralTokenPrice = oracle.getUnderlyingPrice(address(collateralVToken));
        console.log("collateralTokenPrice", collateralTokenPrice);
        uint256 chainCoinPrice = oracle.getUnderlyingPrice(address(0xA07c5b74C9B40447a954e1466938b865b6BBea36));
        console.log("chainCoinPrice", chainCoinPrice);
    }

    function maybeApproveRepayTokenAndLog(
        bool isRepayChainCoin,
        VTokenInterface repayVToken,
        uint256 approveAmount
    )
        private
    {
        (bool needApprove, uint256 gasUsedForApprove) =
            maybeApproveRepayToken(isRepayChainCoin, repayVToken, approveAmount);
        if (needApprove) {
            console.log("gasUsedForApprove", gasUsedForApprove);
        } else {
            console.log("Repay chain coin does not need to be approved");
        }
    }

    function maybeApproveRepayToken(
        bool isRepayChainCoin,
        VTokenInterface repayVToken,
        uint256 approveAmount
    )
        private
        returns (bool needApprove, uint256 gasUsed)
    {
        needApprove = !isRepayChainCoin;
        if (!needApprove) {
            return (needApprove, gasUsed);
        }

        address approveAddress = address(repayVToken);
        address liquidator = getLiquidatorContract();
        if (liquidator != address(0)) {
            approveAddress = liquidator;
        }

        EIP20Interface underlyingToken = EIP20Interface(repayVToken.underlying());
        uint256 gasBefore = gasleft();
        underlyingToken.approve(approveAddress, approveAmount);
        uint256 gasAfter = gasleft();
        gasUsed = gasBefore - gasAfter - 10;
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
        bool isRepayChainCoin,
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
            callLiquidate(repayVToken, borrower, closeFactorRepayAmount, collateralVToken, isRepayChainCoin);

            (vars.allowResult,,) = callLiquidateWithResult(repayVToken, borrower, 1, collateralVToken, isRepayChainCoin);

            vm.revertTo(vars.loopSnapshotId);

            if (vars.allowResult == 0) {
                console.log("Max repayAmount will keep the borrower liquidatable!!!");
                return closeFactorRepayAmount;
            }
        }

        while (vars.low <= vars.high) {
            vars.mid = (vars.low + vars.high) / 2;

            callLiquidate(repayVToken, borrower, vars.mid, collateralVToken, isRepayChainCoin);

            // Want to check if later we are able to repay at the very least closeFactorRepayAmount-repayAmount
            (vars.allowResult,,) = callLiquidateWithResult(
                repayVToken, borrower, closeFactorRepayAmount - vars.mid, collateralVToken, isRepayChainCoin
            );

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

    function findBorrowBalance(VTokenInterface repayVToken, address borrower) private returns (uint256 borrowBalance) {
        uint256 snapshotId = vm.snapshot();
        borrowBalance = VTokenInterface(repayVToken).borrowBalanceCurrent(borrower);
        vm.revertTo(snapshotId);
    }

    function findCloseFactorRepayAmount(
        VTokenInterface repayVToken,
        VTokenInterface vTokenCollateral,
        address borrower
    )
        private
        returns (uint256 repayAmount)
    {
        uint256 functionSnapshot = vm.snapshot();
        uint256 borrowBalance = VTokenInterface(repayVToken).borrowBalanceCurrent(borrower);
        repayAmount = borrowBalance / 2; // Take 50%
        if (compareStrings(repayVToken.symbol(), "vBNB")) {
            // Repay amount increases the cash before the liquidation happens
            // In turn it increases the exchange rate which means the borrowwers borrow balance increases to our benefit
            // But if the borrower has a lot of collateral the increased exchange rate could make the borrower healty.
            // If the borrower has significantly more borrow than collateral then the maximum repay amount by close
            // factor is greater than the current borrow balance /2.

            // We assume that the maximum repay amount lies between [0, borrowBalance]
            // If not then we will later revert.
            dealUnderlyingTokens(repayVToken, borrowBalance);
            uint256 loopSnapshot = vm.snapshot();
            uint256 left = 0;
            uint256 right = borrowBalance;

            if (right == 0) {
                vm.revertTo(functionSnapshot);
                return 0;
            }

            while (left <= right) {
                uint256 mid = (left + right) / 2;

                (uint256 errorCode,,) = callLiquidateWithResult(repayVToken, borrower, mid, vTokenCollateral, true);
                if (errorCode == 0) left = mid + 1;
                else right = mid - 1;
                vm.revertTo(loopSnapshot);
            }
            repayAmount = left - 1;
        }
        vm.revertTo(functionSnapshot);
    }

    function findCollateralCappedRepayAmount(
        VTokenInterface vTokenBorrowed,
        VTokenInterface vTokenCollateral,
        address borrower
    )
        private
        returns (uint256 repayAmount)
    {
        Exp memory ratio = findBorrowToCollateralRatio(vTokenBorrowed, vTokenCollateral);

        uint256 snapshotId = vm.snapshot();
        uint256 collateralBalance = vTokenCollateral.balanceOf(borrower);
        vm.revertTo(snapshotId);

        // repayAmount = div_(collateralBalance, ratio) + ((1e18 - 1) / ratio.mantissa); // Include overpay
        repayAmount = div_roundingUp(collateralBalance + 1, ratio) - 1; // Include overpay
    }

    function adjustRepayAmount(
        VTokenInterface vTokenBorrowed,
        VTokenInterface vTokenCollateral,
        uint256 repayAmount,
        address borrower
    )
        private
        returns (uint256)
    {
        if (repayAmount == 0) {
            return 0;
        }

        // Ensure that the effective repay amount gives the same collateral as the original repay amount
        uint256 collateralGainedWithOriginalRepayAmount;
        {
            uint256 snapshot = vm.snapshot();
            bool isRepayChainCoin = dealUnderlyingTokens(vTokenBorrowed, repayAmount);
            maybeApproveRepayToken(isRepayChainCoin, vTokenBorrowed, repayAmount);
            (collateralGainedWithOriginalRepayAmount,) =
                callLiquidate(vTokenBorrowed, borrower, repayAmount, vTokenCollateral, isRepayChainCoin);
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
        uint256 collateralGainedWithOptimalAmountInMinus;
        if (effectiveRepayAmount > 1) {
            uint256 snapshot = vm.snapshot();
            bool isRepayChainCoin = dealUnderlyingTokens(vTokenBorrowed, effectiveRepayAmount - 1);
            maybeApproveRepayToken(isRepayChainCoin, vTokenBorrowed, effectiveRepayAmount - 1);
            (collateralGainedWithOptimalAmountInMinus,) =
                callLiquidate(vTokenBorrowed, borrower, effectiveRepayAmount - 1, vTokenCollateral, isRepayChainCoin);
            vm.revertTo(snapshot);
        }

        uint256 collateralGainedWithOptimalAmountIn;
        if (effectiveRepayAmount > 0) {
            uint256 snapshot = vm.snapshot();
            bool isRepayChainCoin = dealUnderlyingTokens(vTokenBorrowed, effectiveRepayAmount);
            maybeApproveRepayToken(isRepayChainCoin, vTokenBorrowed, effectiveRepayAmount);
            (collateralGainedWithOptimalAmountIn,) =
                callLiquidate(vTokenBorrowed, borrower, effectiveRepayAmount, vTokenCollateral, isRepayChainCoin);
            vm.revertTo(snapshot);
        }

        assertEq(
            collateralGainedWithOptimalAmountIn,
            collateralGainedWithOriginalRepayAmount,
            "Original repay amount should give the same collateral as effective repay amount"
        );
        if (effectiveRepayAmount > 0) {
            assertGt(
                collateralGainedWithOptimalAmountIn,
                collateralGainedWithOptimalAmountInMinus,
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
        // find the seize ratio that's used
        /* Read oracle prices for borrowed and collateral markets */
        uint256 priceBorrowedMantissa =
            ComptrollerInterface(comptroller).oracle().getUnderlyingPrice(address(vTokenBorrowed));
        uint256 priceCollateralMantissa =
            ComptrollerInterface(comptroller).oracle().getUnderlyingPrice(address(vTokenCollateral));
        require(priceBorrowedMantissa != 0 && priceCollateralMantissa != 0);

        /*
         * Get the exchange rate and calculate the number of collateral tokens to seize:
         *  seizeAmount = actualRepayAmount * liquidationIncentive * priceBorrowed / priceCollateral
         *  seizeTokens = seizeAmount / exchangeRate
         *   = actualRepayAmount * (liquidationIncentive * priceBorrowed) / (priceCollateral * exchangeRate)
         */
        uint256 exchangeRateMantissa = vTokenCollateral.exchangeRateCurrent();
        Exp memory numerator;
        Exp memory denominator;

        numerator = mul_(
            Exp({ mantissa: ComptrollerInterface(comptroller).liquidationIncentiveMantissa() }),
            Exp({ mantissa: priceBorrowedMantissa })
        );
        denominator = mul_(Exp({ mantissa: priceCollateralMantissa }), Exp({ mantissa: exchangeRateMantissa }));
        ratio = div_(numerator, denominator);
        vm.revertTo(snapshotId);
    }

    function findMaxRepayAmount(
        VTokenInterface vTokenBorrowed,
        VTokenInterface vTokenCollateral,
        address borrower
    )
        private
        returns (uint256 repayAmount, bool cappedByCollateral)
    {
        uint256 repayAmountCappedByCloseFactor = findCloseFactorRepayAmount(vTokenBorrowed, vTokenCollateral, borrower);
        // console.log("repayAmountCappedByCloseFactor", repayAmountCappedByCloseFactor);

        uint256 repayAmountCappedByCollateral =
            findCollateralCappedRepayAmount(vTokenBorrowed, vTokenCollateral, borrower);
        // console.log("repayAmountCappedByCollateral ", repayAmountCappedByCollateral);

        if (repayAmountCappedByCollateral < repayAmountCappedByCloseFactor) {
            repayAmount = repayAmountCappedByCollateral;
            cappedByCollateral = true;
        } else {
            repayAmount = repayAmountCappedByCloseFactor;
        }

        {
            // Try to liquidate with 1 wei more - it should fail
            uint256 snapshot = vm.snapshot();
            bool isRepayChainCoin = dealUnderlyingTokens(vTokenBorrowed, repayAmount + 1);
            maybeApproveRepayToken(isRepayChainCoin, vTokenBorrowed, repayAmount + 1);
            callLiquidateExpectFail(vTokenBorrowed, borrower, repayAmount + 1, vTokenCollateral, isRepayChainCoin);
            vm.revertTo(snapshot);
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
            assertEq(treasuryPercentMantissa, 50_000_000_000_000_000, "Treasury percent is not 10%");
        }

        assertWeDontHaveVCollateral(collateralVToken);

        vm.revertTo(snapshot);
    }

    function assertWeDontHaveVCollateral(VTokenInterface collateralVToken) private view {
        assertEq(collateralVToken.balanceOf(myAddress), 0);
    }

    function dealUnderlyingTokens(VTokenInterface vToken, uint256 amount) private returns (bool isChainCoin) {
        isChainCoin = compareStrings(vToken.symbol(), "vBNB");
        if (isChainCoin) {
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

    function callLiquidate(
        VTokenInterface repayVToken,
        address borrower,
        uint256 repayAmount,
        VTokenInterface collateralVToken,
        bool isRepayChainCoin
    )
        private
        returns (uint256 collateralVTokenGained, uint256 gasUsed)
    {
        uint256 errorCode;
        (errorCode, collateralVTokenGained, gasUsed) =
            callLiquidateWithResult(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
        require(errorCode == 0, "Liquidation failed");
    }

    function callLiquidateExpectFail(
        VTokenInterface repayVToken,
        address borrower,
        uint256 repayAmount,
        VTokenInterface collateralVToken,
        bool isRepayChainCoin
    )
        private
    {
        (uint256 errorCode,,) =
            callLiquidateWithResult(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
        require(errorCode != 0, "Expected the liquidation to fail");
    }

    function callLiquidateWithResult(
        VTokenInterface repayVToken,
        address borrower,
        uint256 repayAmount,
        VTokenInterface collateralVToken,
        bool isRepayChainCoin
    )
        private
        returns (uint256 errorCode, uint256 collateralVTokenGained, uint256 gasUsed)
    {
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
            collateralVTokenGained = collateralVToken.balanceOf(myAddress) - collateralVTokenGained;
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
            collateralVTokenGained = collateralVToken.balanceOf(myAddress) - collateralVTokenGained;
        }
    }

    function redeemCollateral(VTokenInterface vToken, uint256 amount) private {
        uint256 exchangeRate;
        {
            uint256 snapshotId = vm.snapshot();
            exchangeRate = vToken.exchangeRateCurrent();
            console.log("collateralExchangeRate", exchangeRate);
            vm.revertTo(snapshotId);
        }

        {
            uint256 treasuryPercent = comptroller.treasuryPercent();
            console.log("treasuryPercent", treasuryPercent);
        }

        uint256 cash;
        {
            uint256 snapshotId = vm.snapshot();
            cash = vToken.getCash();
            console.log("cashInVToken", cash);
            vm.revertTo(snapshotId);
        }

        uint256 predictedUnderlying = amount * exchangeRate / 1e18;
        // console.log("predictedUnderlying", predictedUnderlying);
        bool enoughCash = cash >= predictedUnderlying;
        console.log("enoughCash", enoughCash);

        if (!enoughCash) {
            amount = cash * 1e18 / exchangeRate;
        }

        uint256 collateralBalanceBefore;
        EIP20Interface collateralUnderlyingToken;
        bool isCollateralChainCoin = compareStrings(vToken.symbol(), "vBNB");
        if (isCollateralChainCoin) {
            collateralBalanceBefore = myAddress.balance;
        } else {
            collateralUnderlyingToken = EIP20Interface(vToken.underlying());
            collateralBalanceBefore = collateralUnderlyingToken.balanceOf(myAddress);
        }
        uint256 redeemGasBefore = gasleft();
        uint256 success = VBep20Interface(address(vToken)).redeem(amount);
        uint256 redeemGasAfter = gasleft();
        assertEq(success, 0, "Redeem failed");
        uint256 gasUsed = redeemGasBefore - redeemGasAfter - 10;
        uint256 collateralBalanceAfter;
        if (isCollateralChainCoin) collateralBalanceAfter = myAddress.balance;
        else collateralBalanceAfter = collateralUnderlyingToken.balanceOf(myAddress);
        uint256 collateralUnderlyingGained = collateralBalanceAfter - collateralBalanceBefore;
        console.log("gasUsedForRedeem", gasUsed);
        console.log("collateralUnderlyingSeized", collateralUnderlyingGained);
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
