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
                // The expected seize we parsed from the events is before the treasury cut
                expectedSeize -= expectedSeize / 22; // (seizedAmount * treasuryPercentMantissa) / totalIncentive;
            }
        }

        {
            uint256 treasuryPercent = comptroller.treasuryPercent();
            console.log("treasuryPercent", treasuryPercent);
        }

        // #1 Repeat
        repeatLiquidation(borrower, repayVToken, collateralVToken, repayAmount, expectedSeize);
        vm.revertTo(freshSnapshotId);

        // #2 Up to close factor
        upToCloseFactorLiquidation(borrower, repayVToken, collateralVToken, expectedSeize);
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

        // #6 From largest collateral factor
        // This should work better than smallest
        fromLargestCollateralFactor(borrower);
        vm.revertTo(freshSnapshotId);

        // // #7 From smallest collateral factor
        // fromLargestCollateralFactor(borrower);
        // vm.revertTo(freshSnapshotId);
    }

    function repeatLiquidation(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        uint256 repayAmount,
        uint256 expectedSeize
    )
        private
    {
        console.log("");
        console.log("Tests case: repeatLiquidation");
        logHealthFactor(borrower, repayVToken, collateralVToken);

        bool isRepayChainCoin = dealUnderlyingTokens(repayVToken, repayAmount + 1);
        maybeApproveRepayTokenAndLog(isRepayChainCoin, repayVToken, repayAmount);

        console.log("repayAmount", repayAmount);
        (uint256 collateralVTokenGained, uint256 gasUsed) =
            callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);

        console.log("collateralVTokenGained", collateralVTokenGained);
        console.log("gasUsedForLiquidation", gasUsed);
        logHealthFactor(borrower, repayVToken, collateralVToken);
        assertEq(collateralVTokenGained, expectedSeize, "expected seize does not match");

        redeemCollateral(collateralVToken, collateralVTokenGained);
        logOraclePrices(repayVToken, collateralVToken);

        console.log("Tests case end");
    }

    function upToCloseFactorLiquidation(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        uint256 originalSeizeAmount
    )
        private
    {
        console.log("");
        console.log("Tests case: upToCloseFactorLiquidation");
        upToCloseFactorLiquidationInternal(borrower, repayVToken, collateralVToken, originalSeizeAmount);
        console.log("Tests case end");
    }

    function upToCloseFactorLiquidationInternal(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken,
        uint256 originalSeizeAmount
    )
        private
    {
        logHealthFactor(borrower, repayVToken, collateralVToken);

        // Todo: repay amount pamazinti pagal tai kiek yra cash, manau galim i bendra funkcija perkelti sia logika
        (uint256 repayAmount, bool cappedByCollateral) = findMaxRepayAmount(repayVToken, collateralVToken, borrower);
        console.log("maxRepayAmount", repayAmount);
        console.log("cappedByCollateral", cappedByCollateral);

        repayAmount = findSmallestEffectiveRepayAmount(repayVToken, collateralVToken, repayAmount, borrower);
        if (repayAmount == 0) {
            revert("No collateral to be gained!!!");
        }

        bool isRepayChainCoin = dealUnderlyingTokens(repayVToken, repayAmount + 1);
        maybeApproveRepayTokenAndLog(isRepayChainCoin, repayVToken, repayAmount);

        console.log("repayAmount", repayAmount);
        uint256 collateralVTokenGained;
        uint256 gasUsed;
        if (repayAmount > 0) {
            (collateralVTokenGained, gasUsed) =
                callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
        }

        console.log("collateralVTokenGained", collateralVTokenGained);
        console.log("gasUsedForLiquidation", gasUsed);
        logHealthFactor(borrower, repayVToken, collateralVToken);
        assertGe(
            collateralVTokenGained,
            originalSeizeAmount,
            "Expected to get greater or equal collateral than the original liquidation"
        );

        redeemCollateral(collateralVToken, collateralVTokenGained);
        logOraclePrices(repayVToken, collateralVToken);
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
        drainLiquidationInternal(borrower, repayVToken, collateralVToken);
        console.log("Tests case end");
    }

    function drainLiquidationInternal(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken
    )
        private
    {
        uint256 borrowBalance = findBorrowBalance(repayVToken, borrower);
        bool isRepayChainCoin = dealUnderlyingTokens(repayVToken, borrowBalance + 1);
        maybeApproveRepayTokenAndLog(isRepayChainCoin, repayVToken, borrowBalance);

        uint256 repayAmountTotal = 0;
        uint256 liquidationCount = 1;
        while (true) {
            // TODO: after every liquidation make sure it's worth it for the gas spent
            (uint256 maxRepayAmount, bool cappedByCollateral) =
                findMaxRepayAmount(repayVToken, collateralVToken, borrower);
            if (maxRepayAmount <= 1 || cappedByCollateral) break;

            uint256 repayAmountThatKeepsBorrowerLiquidatable = findRepayAmountThatKeepsBorrowerLiquidatable(
                repayVToken, borrower, collateralVToken, isRepayChainCoin, maxRepayAmount
            );

            uint256 repayAmount = findSmallestEffectiveRepayAmount(
                repayVToken, collateralVToken, repayAmountThatKeepsBorrowerLiquidatable, borrower
            );
            if (repayAmount == 0) break;

            logHealthFactor(borrower, repayVToken, collateralVToken);
            console.log(liquidationCount, "repayAmount", repayAmount);
            repayAmountTotal += repayAmount;
            (uint256 collateralVTokenGained, uint256 gasUsed) =
                callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
            console.log(liquidationCount, "collateralVTokenGained", collateralVTokenGained);
            console.log(liquidationCount, "gasUsedForLiquidation", gasUsed);
            liquidationCount++;
            if (collateralVTokenGained == 0) {
                // Should never happen after findSmallestEffectiveRepayAmount was implemented
                revert("collateral gained cannot be zero");
            }
            if (repayAmountThatKeepsBorrowerLiquidatable < maxRepayAmount) break;
        }

        // (Maybe) last liquidation
        (uint256 repayAmount, bool cappedByCollateral) = findMaxRepayAmount(repayVToken, collateralVToken, borrower);
        repayAmount = findSmallestEffectiveRepayAmount(repayVToken, collateralVToken, repayAmount, borrower);
        if (repayAmount > 0) {
            // console.log("Last liquidation needed", true);
            logHealthFactor(borrower, repayVToken, collateralVToken);
            console.log(liquidationCount, "repayAmount", repayAmount);
            repayAmountTotal += repayAmount;
            (uint256 collateralVTokenGained, uint256 gasUsed) =
                callLiquidate(repayVToken, borrower, repayAmount, collateralVToken, isRepayChainCoin);
            console.log(liquidationCount, "collateralVTokenGained", collateralVTokenGained);
            console.log(liquidationCount, "gasUsedForLiquidation", gasUsed);
        }

        console.log("cappedByCollateral", cappedByCollateral);

        console.log("repayAmountTotal", repayAmountTotal);
        uint256 collateralVTokenGainedTotal = collateralVToken.balanceOf(myAddress);
        console.log("collateralVTokenGainedTotal", collateralVTokenGainedTotal);
        logHealthFactor(borrower, repayVToken, collateralVToken);

        redeemCollateral(collateralVToken, collateralVTokenGainedTotal);
        logOraclePrices(repayVToken, collateralVToken);
    }

    struct AssetData {
        VTokenInterface vtoken;
        string symbol;
        uint256 collateralFactor;
        uint256 collateralAmount;
        uint256 borrowAmount;
        uint256 cash;
        uint256 exchangeRate;
        uint256 price;
    }

    function largestBorrow(address borrower) private {
        console.log("");
        console.log("Tests case: largestBorrow");

        AssetData[] memory assets = getAssets(borrower);
        // logAssets(assets);

        (AssetData memory repayAsset, AssetData memory collateralAsset) = pickRepayAndCollateralAssets(assets);
        console.log("repayAsset", address(repayAsset.vtoken));
        console.log("collateralAsset", address(collateralAsset.vtoken));

        VTokenInterface repayVToken = repayAsset.vtoken;
        VTokenInterface collateralVToken = collateralAsset.vtoken;

        upToCloseFactorLiquidationInternal(borrower, repayVToken, collateralVToken, 0);

        console.log("Tests case end");
    }

    function drainSameToken(address borrower) private {
        console.log("");
        console.log("Tests case: drainSameToken");

        AssetData[] memory assets = getAssets(borrower);
        assets = pickAssetsThatAreBorrowedAndStaked(assets);
        for (uint256 i = 0; i < assets.length; i++) {
            console.log("sameToken case", i);
            VTokenInterface repayVToken = assets[i].vtoken;
            console.log("repayVToken", address(repayVToken));
            drainLiquidationInternal(borrower, repayVToken, repayVToken);
            console.log("sameToken case end");
        }

        if (assets.length == 0) {
            console.log("there are no tokens that are both borrowed and staked");
        }
        console.log("Tests case end");
    }

    struct LiquidationInfo {
        // Fix
        AssetData repayAsset;
        AssetData collateralAsset;
        uint256 collateralFactor;
        uint256 collateralAmount;
        uint256 borrowAmount;
        uint256 cash;
        uint256 exchangeRate;
        uint256 price;
    }

    function fromLargestCollateralFactor(address borrower) private {
        console.log("");
        console.log("Tests case: fromLargestCollateralFactor");

        AssetData[] memory assets = getAssets(borrower);
        logAssets(assets);

        // At every iteration find the L_max
        (AssetData memory repayAsset, AssetData memory collateralAsset) = pickRepayAndCollateralAssets(assets);
        VTokenInterface repayVToken = repayAsset.vtoken;
        VTokenInterface collateralVToken = collateralAsset.vtoken;
        console.log("repayVToken", address(repayVToken));
        console.log("collateralVToken", address(collateralVToken));
        // Suzinome koks yra collateral_value

        // Dabar parenkam L_i su:
        // collateral - didziausias collateral factor. Jeigu yra vienodi tai ta kurio collateral yra daugiau
        // borrow - didziausia skola

        // jeigu sutampa su max ka daryti?
        // galetu praversti pirma sulikviduoti non max tokenus

        // repay amount parenkam toki, kad L_max_po_i.collateral_value - L_i.collateral_value
        // butu > L_max.collateral_value

        // Skolos tokena parinkti ta su didziausia skola
        // jeigu L_max yra ribojamas collateral tai galime pervirsi skolos
        // paskirti L_i ieskojimui be jokios itakos L_maxui

        // lygiai taip pat su collateral tokenu - jeigu L_max yra ribojamas skolos tai
        // pervirsinis uzstast gali buti skirtas L_i ieskojimui

        // tai dabar reikia iskoti tokiu L_i, kuriu profit padengtu execution gas
        // ir tokius, kad imame tas collateral tokenus, kuriu liquidation threashold maziausias
        // Best to group the assets to borrow and collateral in some order maybe

        // Ciklo pradzioj randam L_max.
        // insert code to finde the L_max
        // L_max definition - skola su didziausia skola
        // LiquidationInfo memory lMax = findMaxLiquidation(assets);

        // Po L_i norim galeti likviduoti bent L_max.collateral_value - L_i.collateral_value

        // Kaip parinkti collateral tokena tai aisku, bet kaip parinkti repay.
        // Norim didziausia skola pasilikti galui, kad butu galima likviduoti kuo daugiau
        // Jeigu esam ribojami uzstato kiekiu tai galim imti ir is main repay valiutos.

        // assets = pickAssetsThatAreBorrowedAndStaked(assets);
        // for (uint256 i = 0; i < assets.length; i++) {
        //     console.log("sameToken case", i);
        //     VTokenInterface repayVToken = assets[i].vtoken;
        //     console.log("repayVToken", address(repayVToken));
        //     drainLiquidationInternal(borrower, repayVToken, repayVToken);
        // }

        console.log("Tests case end");
    }

    function logHealthFactor(
        address borrower,
        VTokenInterface repayVToken,
        VTokenInterface collateralVToken
    )
        private
        returns (uint256)
    {
        uint256 snapshot = vm.snapshot();
        repayVToken.accrueInterest();
        collateralVToken.accrueInterest();

        (uint256 borrowCapacity, uint256 borrowed) = getAccountLiquidity(borrower);

        console.log("healthFactor", ratioToString(borrowCapacity, borrowed));

        vm.revertTo(snapshot);
        return 0;
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

    function getAccountLiquidity(address account) private view returns (uint256, uint256) {
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
            try vaiController.getVAIRepayAmount(account) returns (uint256 vaiDebt) {
                vars.sumBorrowPlusEffects = add_(vars.sumBorrowPlusEffects, vaiDebt);
            } catch {
                // The getVAIRepayAmount got implemented only later
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
        return tempResult;
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
            if (collateralValue > assets[i].cash) {
                collateralValue = assets[i].cash;
            }
            collateralValue = collateralValue * asset.price / 1e18;
            if (collateralValue > largestCollateralValue) {
                largestCollateralValue = collateralValue;
                largestCollateralAsset = asset;
            }

            uint256 borrowValue = assets[i].borrowAmount * asset.price / 1e18;
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
            console.log("borrowAssetHasEnoughCollateral!!!!");
            collateralAsset = repayAsset;
        } else {
            collateralAsset = largestCollateralAsset;
        }

        return (repayAsset, collateralAsset);
    }

    function getAssets(address borrower) private returns (AssetData[] memory) {
        // TODO: take a snapshot
        address[] memory vtokens = comptroller.getAssetsIn(borrower);
        AssetData[] memory assets = new AssetData[](vtokens.length);
        uint256 validAssetCount = 0;

        PriceOracle oracle = comptroller.oracle();
        for (uint256 i = 0; i < vtokens.length; i++) {
            VTokenInterface vtoken = VTokenInterface(vtokens[i]);
            vtoken.accrueInterest();

            AssetData memory asset;
            asset.vtoken = vtoken;
            asset.symbol = vtoken.symbol();
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

    // Jeigu visi vienodo ordering tai pazymekim ir neleiskim reverse
    function orderAssetsByAscendingCollateralFactor(AssetData[] memory) private returns (AssetData[] memory) { }

    function logAssets(AssetData[] memory assets) private pure {
        for (uint256 i = 0; i < assets.length; i++) {
            console.log("assetIndex", i);
            console.log("vtoken", address(assets[i].vtoken));
            console.log("symbol", assets[i].symbol);
            console.log("collateralFactor", assets[i].collateralFactor);
            console.log("collateralAmount", assets[i].collateralAmount);
            console.log(
                "collateralValueUsd",
                assets[i].collateralAmount * assets[i].exchangeRate / 1e18 * assets[i].price / 1e36
            );
            console.log("borrowAmount", assets[i].borrowAmount);
            console.log("borrowValueUsd", assets[i].borrowAmount * assets[i].price / 1e36);
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
            assertEq(treasuryPercentMantissa, 50_000_000_000_000_000, "Liquidator treasury percent is not 5%");
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
        uint256 treasuryPercent;
        uint256 cash;
        {
            uint256 snapshotId = vm.snapshot();

            exchangeRate = vToken.exchangeRateCurrent();
            console.log("collateralExchangeRate", exchangeRate);
            treasuryPercent = comptroller.treasuryPercent();
            console.log("treasuryPercent", treasuryPercent);
            cash = vToken.getCash();
            console.log("cashInVToken", cash);

            vm.revertTo(snapshotId);
        }

        uint256 predictedUnderlying = amount * exchangeRate / 1e18;
        bool enoughCash = cash >= predictedUnderlying;
        console.log("enoughCash", enoughCash);

        if (!enoughCash) {
            revert("not enough cash to redeem");
            // TODO: gali buti reikes nuimti revert nes kartais apsimokes gauti daugiau vtoken nei yra cash
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
