#!/bin/sh
#set -o errexit -o nounset -o pipefail

if [ -z "$PASSWORD" ]
then
    echo -n "Enter passphrase:"
    read -s PASSWORD
fi 

CHAIN_ID=${CHAIN_ID:-Oraichain}
USER=${USER:-tupt}
MONIKER=${MONIKER:-node001}

# mannually reset
# rm -rf "$PWD"/.$DENOM/*
if [ ! -d "$PWD/.$DENOM/data" ]
then     
    oraid init --chain-id $CHAIN_ID "$MONIKER"

    (echo "$PASSWORD"; echo "$PASSWORD") | oraid keys add $USER 2>&1 | tee ibc/accounts/$CHAIN_ID.txt

    # hardcode the validator account for this instance
    (echo "$PASSWORD") | oraid add-genesis-account $USER "100000000000000$DENOM"

    # submit a genesis validator tx
    ## Workraround for https://github.com/cosmos/cosmos-sdk/issues/8251
    (echo "$PASSWORD"; echo "$PASSWORD") | oraid gentx $USER "$AMOUNT$DENOM" --chain-id=$CHAIN_ID --amount="$AMOUNT" -y

    oraid collect-gentxs

    oraid validate-genesis

    # cat $PWD/.oraid/config/genesis.json | jq .app_state.genutil.gen_txs[0] -c > "$MONIKER"_validators.txt

    echo "The genesis initiation process has finished ..."
fi

oraid start --rpc.laddr tcp://0.0.0.0:26657 --log_level error

