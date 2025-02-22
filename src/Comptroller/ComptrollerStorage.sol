// SPDX-License-Identifier: Unlicense
pragma solidity ^0.6.2;

interface ComptrollerTypes {
    enum Action {
        MINT,
        REDEEM,
        BORROW,
        REPAY,
        SEIZE,
        LIQUIDATE,
        TRANSFER,
        ENTER_MARKET,
        EXIT_MARKET
    }
}
