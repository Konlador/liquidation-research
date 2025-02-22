// SPDX-License-Identifier: Unlicense
pragma solidity ^0.6.2;

// import "../Tokens/VTokens/VToken.sol";
import "../Oracle/PriceOracle.sol";
// import "../Tokens/VAI/VAIControllerInterface.sol";
import { ComptrollerTypes } from "./ComptrollerStorage.sol";

abstract contract ComptrollerInterface {
    /// @notice Indicator that this is a Comptroller contract (for inspection)
    bool public constant isComptroller = true;

    /**
     * Assets You Are In **
     */
    function enterMarkets(address[] calldata vTokens) external virtual returns (uint256[] memory);

    function exitMarket(address vToken) external virtual returns (uint256);

    /**
     * Policy Hooks **
     */
    function mintAllowed(address vToken, address minter, uint256 mintAmount) external virtual returns (uint256);

    function mintVerify(address vToken, address minter, uint256 mintAmount, uint256 mintTokens) external virtual;

    function redeemAllowed(address vToken, address redeemer, uint256 redeemTokens) external virtual returns (uint256);

    function redeemVerify(
        address vToken,
        address redeemer,
        uint256 redeemAmount,
        uint256 redeemTokens
    )
        external
        virtual;

    function borrowAllowed(address vToken, address borrower, uint256 borrowAmount) external virtual returns (uint256);

    function borrowVerify(address vToken, address borrower, uint256 borrowAmount) external virtual;

    function repayBorrowAllowed(
        address vToken,
        address payer,
        address borrower,
        uint256 repayAmount
    )
        external
        virtual
        returns (uint256);

    function repayBorrowVerify(
        address vToken,
        address payer,
        address borrower,
        uint256 repayAmount,
        uint256 borrowerIndex
    )
        external
        virtual;

    function liquidateBorrowAllowed(
        address vTokenBorrowed,
        address vTokenCollateral,
        address liquidator,
        address borrower,
        uint256 repayAmount
    )
        external
        virtual
        returns (uint256);

    function liquidateBorrowVerify(
        address vTokenBorrowed,
        address vTokenCollateral,
        address liquidator,
        address borrower,
        uint256 repayAmount,
        uint256 seizeTokens
    )
        external
        virtual;

    function seizeAllowed(
        address vTokenCollateral,
        address vTokenBorrowed,
        address liquidator,
        address borrower,
        uint256 seizeTokens
    )
        external
        virtual
        returns (uint256);

    function seizeVerify(
        address vTokenCollateral,
        address vTokenBorrowed,
        address liquidator,
        address borrower,
        uint256 seizeTokens
    )
        external
        virtual;

    function transferAllowed(
        address vToken,
        address src,
        address dst,
        uint256 transferTokens
    )
        external
        virtual
        returns (uint256);

    function transferVerify(address vToken, address src, address dst, uint256 transferTokens) external virtual;

    /**
     * Liquidity/Liquidation Calculations **
     */
    function liquidateCalculateSeizeTokens(
        address vTokenBorrowed,
        address vTokenCollateral,
        uint256 repayAmount
    )
        external
        view
        virtual
        returns (uint256, uint256);

    function setMintedVAIOf(address owner, uint256 amount) external virtual returns (uint256);

    function liquidateVAICalculateSeizeTokens(
        address vTokenCollateral,
        uint256 repayAmount
    )
        external
        view
        virtual
        returns (uint256, uint256);

    function getXVSAddress() public view virtual returns (address);

    function markets(address) external view virtual returns (bool, uint256);

    function oracle() external view virtual returns (PriceOracle);

    function getAccountLiquidity(address) external view virtual returns (uint256, uint256, uint256);

    function getAssetsIn(address) external view virtual returns (address[] memory);

    function claimVenus(address) external virtual;

    function venusAccrued(address) external view virtual returns (uint256);

    function venusSupplySpeeds(address) external view virtual returns (uint256);

    function venusBorrowSpeeds(address) external view virtual returns (uint256);

    function getAllMarkets() external view virtual returns (address[] memory);

    function venusSupplierIndex(address, address) external view virtual returns (uint256);

    function venusInitialIndex() external view virtual returns (uint224);

    function venusBorrowerIndex(address, address) external view virtual returns (uint256);

    function venusBorrowState(address) external view virtual returns (uint224, uint32);

    function venusSupplyState(address) external view virtual returns (uint224, uint32);

    function approvedDelegates(address borrower, address delegate) external view virtual returns (bool);

    function vaiController() external view virtual returns (address);

    function liquidationIncentiveMantissa() external view virtual returns (uint256);

    function protocolPaused() external view virtual returns (bool);

    function actionPaused(address market, ComptrollerTypes.Action action) public view virtual returns (bool);

    function mintedVAIs(address user) external view virtual returns (uint256);

    function vaiMintRate() external view virtual returns (uint256);

    // myFunctions
    function closeFactorMantissa() external view virtual returns (uint256);
    function liquidatorContract() external view virtual returns (address);
    function treasuryPercent() external view virtual returns (uint256);
}

interface IVAIVault {
    function updatePendingRewards() external;
}

interface IComptroller {
    function liquidationIncentiveMantissa() external view returns (uint256);

    /**
     * Treasury Data **
     */
    function treasuryAddress() external view returns (address);

    function treasuryPercent() external view returns (uint256);
}
