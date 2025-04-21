#!/bin/bash

if [ -z "$1" ]; then
  echo "Usage: $0 <transaction_hash>"
  exit 1
fi

TX_HASH_ARG="$1"

DB_NAME="discovery_manager"
DB_USER="postgres"
DB_PASSWORD="root"
DB_HOST="localhost"
DB_PORT="5432"

clear

result=$(PGPASSWORD="$DB_PASSWORD" psql -U "$DB_USER" -d "$DB_NAME" -h "$DB_HOST" -p "$DB_PORT" -t -A -F "|" -c "
SELECT transaction_hash, v_token, borrower, repay_amount, v_token_collateral, seize_tokens
FROM bsc.venus_liquidations
WHERE transaction_hash = '$TX_HASH_ARG'
LIMIT 1;
" | tr -d '[:space:]')

export TX_HASH=$(echo "$result" | awk -F'|' '{print $1}')
export REPAY_V_TOKEN=$(echo "$result" | awk -F'|' '{print $2}')
export BORROWER=$(echo "$result" | awk -F'|' '{print $3}')
export REPAY_AMOUNT=$(echo "$result" | awk -F'|' '{print $4}')
export COLLATERAL_V_TOKEN=$(echo "$result" | awk -F'|' '{print $5}')
export EXPECTED_SEIZE=$(echo "$result" | awk -F'|' '{print $6}')

echo "TX_HASH=$TX_HASH"
echo "REPAY_V_TOKEN=$REPAY_V_TOKEN"
echo "BORROWER=$BORROWER"
echo "REPAY_AMOUNT=$REPAY_AMOUNT"
echo "COLLATERAL_V_TOKEN=$COLLATERAL_V_TOKEN"
echo "EXPECTED_SEIZE=$EXPECTED_SEIZE"

forge test --match-test testLiquidations -vv