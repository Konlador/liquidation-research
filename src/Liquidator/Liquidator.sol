// SPDX-License-Identifier: Unlicense
pragma solidity ^0.6.2;

interface Liquidator {
    function liquidateBorrow(
        address vToken,
        address borrower,
        uint256 repayAmount,
        address vTokenCollateral
    )
        external
        payable;

    function treasuryPercentMantissa() external view returns (uint256);
}
