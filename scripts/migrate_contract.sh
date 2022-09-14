#!/bin/sh
set -o errexit -o nounset -o pipefail

contract_path=$1
contract=$2
migrate=${3:-{\}}

CHAIN_ID=${CHAIN_ID:-Oraichain}

echo "Enter passphrase:"
read -s passphrase

store_ret=$(echo $passphrase | oraid tx wasm store $contract_path --from $USER --gas="auto" --gas-adjustment="1.2" --chain-id=$CHAIN_ID -y)
echo $store_ret
if [ ! `command -v jq` ]; then  
    echo "Installing jq ..."
    [ `uname -s | grep Darwin` ] && brew install jq || apk add jq    
fi  
code_id=$(echo $store_ret | jq -r '.logs[0].events[0].attributes[] | select(.key | contains("code_id")).value')

# echo "oraid tx wasm instantiate $code_id '$init' --from $USER --label '$label' --gas auto --gas-adjustment 1.2 --chain-id=$CHAIN_ID -y"
# quote string with "" with escape content inside which contains " characters
(echo $passphrase;echo $passphrase) | oraid tx wasm migrate $contract $code_id $migrate --from $USER --gas auto --gas-adjustment 1.2 --chain-id=$CHAIN_ID -y
contract_address=$(oraid query wasm list-contract-by-code $code_id -o json | jq -r '.contracts[-1]')

echo "contract address: $contract_address"