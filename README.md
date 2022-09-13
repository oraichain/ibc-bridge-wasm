## Installation

```bash
docker compose up -d
```

## deploy smart contract

```bash
# build smart contract
./scripts/build_contract.sh contracts/cw20-ics20
cp contracts/cw20-ics20/artifacts/cw20-ics20.wasm .mars

# go to mars network
docker compose exec mars ash
./scripts/deploy_contract.sh .mars/cw20-ics20.wasm 'cw20-ics20' '{"default_timeout":30}'

```

## start relayer

```bash
hermes --config config.toml keys add --chain Earth --mnemonic-file accounts/Earth.txt
hermes --config config.toml keys add --chain Mars --mnemonic-file accounts/Mars.txt

# create a channel
hermes --config config.toml create channel --a-chain Earth --b-chain Mars --a-port transfer --b-port wasm.mars1pcknsatx5ceyfu6zvtmz3yr8auumzrdttv7lzx --new-client-connection

# start hermes
hermes --config config.toml start
```

## send cross-channel

```bash
# from earth to mars on channel
docker compose exec earth ash
oraid tx ibc-transfer transfer transfer channel-1 mars1v2rtl3zzrhtaywnu96fyj4j90mzd390u0wfu9n 10000000earth --from duc --chain-id Earth -y
# check mars balance
docker compose exec mars ash
oraid query bank balances mars1v2rtl3zzrhtaywnu96fyj4j90mzd390u0wfu9n

```
