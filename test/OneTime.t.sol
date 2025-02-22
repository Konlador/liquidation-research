// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.6.2;

import { Test } from "forge-std/src/Test.sol";
import { console2 } from "forge-std/src/console2.sol";

import { LiquidationTest } from "./Foo.t.sol";
import { ComptrollerInterface } from "../src/Comptroller/ComptrollerInterface.sol";

//contract OneTime is FooTest {
// function testForkIdDiffer() public {
//     assert(realFork != closeFactorFork);
// }

// function testGetAllMarkets() public {
//     vm.createSelectFork(QUICKNODE_RPC_URL);
//     address[] memory allMarkets = comptroller.getAllMarkets();
//     for (uint256 i = 0; i < allMarkets.length; i++) {
//         console2.log(allMarkets[i]);
//     }
//     console2.log("Done");
// }

// /// @dev Basic test. Run it with `forge test -vvv` to see the console log.
// function test_Example() external view {
//     console2.log("Hello World");
//     uint256 x = 42;
//     assertEq(foo.id(x), x, "value mismatch");
// }

// /// @dev Fuzz test that provides random values for an unsigned integer, but which rejects zero as an input.
// /// If you need more sophisticated input validation, you should use the `bound` utility instead.
// /// See https://twitter.com/PaulRBerg/status/1622558791685242880
// function testFuzz_Example(uint256 x) external view {
//     vm.assume(x != 0); // or x = bound(x, 1, 100)
//     assertEq(foo.id(x), x, "value mismatch");
// }
//}
