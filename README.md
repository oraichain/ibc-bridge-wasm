## Installation

```bash

# run the build container for oraid. Wait for it to finish
docker compose -f docker-compose.build.yml up
```

## Start the networks

```bash
docker-compose up -d
```

## deploy smart contract

```bash
# build smart contract
./scripts/build_contract.sh contracts/cw20-ics20
cp contracts/cw20-ics20/artifacts/cw20-ics20.wasm .mars

# build cw20
./scripts/build_contract.sh contracts/cw20-base
cp contracts/cw20-base/artifacts/cw20-base.wasm .mars

# go to mars network
docker compose exec mars ash
./scripts/deploy_contract.sh .mars/cw20-ics20.wasm 'cw20-ics20' '{"default_timeout":90}'
./scripts/deploy_contract.sh .mars/cw20-base.wasm 'cw20-base' '{"name":"EARTH","symbol":"EARTH","decimals":6,"initial_balances":[{"address":"mars15nr8gcygpn9pq8urkzcf6hvzzvf0s2qt7d5z94","amount":"100000000000000"}],"mint":{"minter":"mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq"}}'

# migrate contract
./scripts/migrate_contract.sh .mars/cw20-ics20.wasm mars15nr8gcygpn9pq8urkzcf6hvzzvf0s2qt7d5z94
```

## start relayer

```bash

docker-compose exec hermes bash
hermes --config config.toml keys add --chain Earth --mnemonic-file accounts/Earth.txt
hermes --config config.toml keys add --chain Mars --mnemonic-file accounts/Mars.txt

# create a channel
hermes --config config.toml create channel --a-chain Earth --b-chain Mars --a-port transfer --b-port wasm.mars15nr8gcygpn9pq8urkzcf6hvzzvf0s2qt7d5z94 --new-client-connection

# start hermes
hermes --config config.toml start
```

## send cross-channel

```bash
# from earth to mars on channel
docker compose exec earth ash
oraid tx ibc-transfer transfer transfer channel-1 mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq 10000000earth --from duc --chain-id Earth -y
# check mars balance
docker compose exec mars ash
oraid query wasm contract-state smart mars199d3u09j0n6ud2g0skevp93utgnp38kdata8s4 '{"balance":{"address":"mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq"}}'

```