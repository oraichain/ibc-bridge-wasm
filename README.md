## Installation

```bash

# run the build container for oraid. Wait for it to finish
docker-compose -f docker-compose.build.yml up
```

## Start the networks

```bash
docker-compose up -d
```

## deploy smart contract

```bash
# build smart contract
./scripts/build_contract.sh contracts/cw-ics20-latest
cp contracts/cw-ics20-latest/artifacts/cw-ics20-latest.wasm .mars

# build cw20
./scripts/build_contract.sh contracts/cw20-base
cp contracts/cw20-base/artifacts/cw20-base.wasm .mars

# go to mars network
docker-compose exec mars ash

./scripts/deploy_contract.sh .mars/cw-ics20-latest.wasm 'cw20-ics20-latest' '{"default_timeout":180,"gov_contract":"mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq","allowlist":[]}'
# mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde

./scripts/deploy_contract.sh .mars/cw20-base.wasm 'cw20-base' '{"name":"EARTH","symbol":"EARTH","decimals":6,"initial_balances":[{"address":"mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde","amount":"100000000000000"}],"mint":{"minter":"mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq"}}'
# mars1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrqhnhf0l

# mint token for cw20-ics20 (optional)
oraid tx wasm execute mars1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrqhnhf0l '{"mint":{"recipient":"mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde","amount":"100000000000000000000000"}}' --keyring-backend test --from $USER --chain-id $CHAIN_ID -y -b block

oraid tx wasm execute mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde '{"update_cw20_mapping_pair":{"dest_ibc_endpoint":{"port_id":"wasm.mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde","channel_id":"channel-0"},"denom":"uusd","cw20_denom":"cw20:mars1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrqhnhf0l","remote_decimals":6,"cw20_decimals":6}}' --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block

# migrate contract
./scripts/migrate_contract.sh .mars/cw-ics20-latest.wasm mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde # migrate to test changing cw20 contract
```

## start relayer

```bash

docker-compose exec hermes bash
hermes --config config.toml keys add --chain Earth --mnemonic-file accounts/Earth.txt
hermes --config config.toml keys add --chain Mars --mnemonic-file accounts/Mars.txt

# create a channel
hermes --config config.toml create channel --a-chain Earth --b-chain Mars --a-port transfer --b-port wasm.mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde --new-client-connection

# start hermes
hermes --config config.toml start
```

## send cross-channel

```bash
# from earth to mars on channel
docker-compose exec earth ash
oraid tx ibc-transfer transfer transfer channel-0 mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq 1earth --from duc --chain-id Earth -y --keyring-backend test -b block
# check mars balance
docker-compose exec mars ash
oraid query wasm contract-state smart mars1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrqhnhf0l '{"balance":{"address":"mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq"}}'

# query channel
oraid query wasm contract-state smart mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde '{"channel":{"id":"channel-0"}}'
```

## test send back to remote chain

```bash

# update the native allow contract to admin so we dont need to call cross contract for testing
oraid tx wasm execute mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde '{"update_native_allow_contract":"mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq"}' --from duc --chain-id $CHAIN_ID -y --keyring-backend test -b block

# call transfer back method
oraid tx wasm execute mars1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrqhnhf0l '{"send":{"amount":"1","contract":"mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde","msg":"'$(echo '{"local_ibc_endpoint":{"port_id":"wasm:mars14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9smxjtde","channel_id":"channel-0"},"remote_address":"earth1w84gt7t7dzvj6qmf5q73d2yzyz35uwc7y8fkwp"}' | base64 -w 0)'"}}' --from mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq --chain-id $CHAIN_ID -y -b block --keyring-backend test
```

# TODO: draw a threat model for this