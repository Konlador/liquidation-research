// SPDX-License-Identifier: MIT
pragma solidity ^0.6.2;

abstract contract VAIControllerInterface {
    function getVAIRepayAmount(address account) external view virtual returns (uint256);
}
