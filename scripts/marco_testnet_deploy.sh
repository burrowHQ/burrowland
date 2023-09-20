#!/bin/bash
set -e

MASTER_ACCOUNT=burrow.testnet
TIME=$(date +%s)

cd "$(dirname $0)/.."

export NEAR_ENV=testnet
LG='\033[1;30m' # Arrows color (Dark gray)
TC='\033[0;33m' # Text color (Orange)
NC='\033[0m' # No Color

echo -e "$LG>>>>>>>>>>>>>>$TC Deploy an empty contract to fund main account $LG<<<<<<<<<<<<<<$NC"
echo -n "" > /tmp/empty
near dev-deploy -f /tmp/empty
TMP_ACCOUNT="$(cat neardev/dev-account)"

MAIN="${TIME}.${MASTER_ACCOUNT}"

echo -e "$LG>>>>>>>>>>>>>>$TC Creating main account: $MAIN $LG<<<<<<<<<<<<<<$NC"
near create-account $MAIN --masterAccount=$MASTER_ACCOUNT --initialBalance=0.01

echo -e "$LG>>>>>>>>>>>>>>$TC Funding main account: $MAIN $LG<<<<<<<<<<<<<<$NC"
near delete $TMP_ACCOUNT $MAIN

OWNER_ID="owner.$MAIN"
echo -e "$LG>>>>>>>>>>>>>>$TC Creating owner account: $OWNER_ID $LG<<<<<<<<<<<<<<$NC"
near create-account $OWNER_ID --masterAccount=$MAIN --initialBalance=130

BOOSTER_TOKEN_ID="token.$MAIN"
echo -e "$LG>>>>>>>>>>>>>>$TC Creating and deploying booster token: $BOOSTER_TOKEN_ID $LG<<<<<<<<<<<<<<$NC"
near create-account $BOOSTER_TOKEN_ID --masterAccount=$MAIN --initialBalance=3
near deploy $BOOSTER_TOKEN_ID res/fungible_token.wasm new '{
   "owner_id": "'$OWNER_ID'",
   "total_supply": "1000000000000000000000000000",
   "metadata": {
       "spec": "ft-1.0.0",
       "name": "Booster Token ('$TIME')",
       "symbol": "BOOSTER-'$TIME'",
       "decimals": 18
   }
}'

ORACLE_ID="priceoracle.testnet"

CONTRACT_ID="contract.$MAIN"

echo -e "$LG>>>>>>>>>>>>>>$TC Creating and deploying contract account: $CONTRACT_ID $LG<<<<<<<<<<<<<<$NC"
near create-account $CONTRACT_ID --masterAccount=$MAIN --initialBalance=10

echo -e "$LG>>>>>>>>>>>>>>$TC Dropping info to continue working from NEAR CLI: $LG<<<<<<<<<<<<<<$NC"
echo -e "export NEAR_ENV=testnet"
echo -e "export BURROW_OWNER=$OWNER_ID"
echo -e "export ORACLE_ID=$ORACLE_ID"
echo -e "export BURROW_CONTRACT=$CONTRACT_ID"
echo -e "export BOOSTER_TOKEN_ID=$BOOSTER_TOKEN_ID"
