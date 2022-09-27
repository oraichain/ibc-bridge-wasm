#!/bin/sh
set -o errexit -o nounset -o pipefail

contract_path=$1
label=$2
init=${3:-{\}}
code_id=${4:-}
CHAIN_ID=${CHAIN_ID:-Oraichain}

if [ -z $code_id ]
then 
    store_ret=$(oraid tx wasm store $contract_path --from $USER --gas="auto" --gas-adjustment="1.2" --chain-id=$CHAIN_ID -y --keyring-backend test)
    echo $store_ret
    if [ ! `command -v jq` ]; then  
        echo "Installing jq ..."
        [ `uname -s | grep Darwin` ] && brew install jq || apk add jq    
    fi  
    code_id=$(echo $store_ret | jq -r '.logs[0].events[0].attributes[] | select(.key | contains("code_id")).value')
fi 

# echo "oraid tx wasm instantiate $code_id '$init' --from $USER --label '$label' --gas auto --gas-adjustment 1.2 --chain-id=$CHAIN_ID -y"
# quote string with "" with escape content inside which contains " characters

admin=$(oraid keys show $USER --output json --keyring-backend test | jq -r '.address')

oraid tx wasm instantiate $code_id "$init" --from $USER --label "$label" --gas auto --gas-adjustment 1.2 --admin $admin --chain-id=$CHAIN_ID -y --keyring-backend test
contract_address=$(oraid query wasm list-contract-by-code $code_id -o json | jq -r '.contracts[-1]')

echo "contract address: $contract_address"
