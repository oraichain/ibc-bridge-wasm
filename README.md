## Installation

```bash
docker compose up -d
```

## start relayer

```bash
hermes --config config.toml keys add --chain Earth --mnemonic-file accounts/Earth.txt
hermes --config config.toml keys add --chain Mars --mnemonic-file accounts/Mars.txt

# create a channel
hermes --config config.toml create channel --a-chain Earth --b-chain Mars --a-port transfer --b-port transfer --channel-version ibc-reflect-v1 --new-client-connection

# start hermes
hermes --config config.toml start
```

## send cross-channel

```bash
# from earth to mars on channel
docker compose exec earth ash
oraid tx ibc-transfer transfer transfer channel-0 mars1v2rtl3zzrhtaywnu96fyj4j90mzd390u0wfu9n 10000000earth --from duc --chain-id Earth -y
# check mars balance
docker compose exec mars ash
oraid query bank balances mars1v2rtl3zzrhtaywnu96fyj4j90mzd390u0wfu9n
```

## deploy smart contract

```bash
# build smart contract
./scripts/build_contract.sh contracts/ibc-reflect
cp contracts/ibc-reflect/artifacts/ibc-reflect.wasm .mars

# go to mars network
docker compose exec mars ash
./scripts/deploy_contract.sh .mars/ibc-reflect.wasm 'ibc-reflect' '{"reflect_code_id":101}'

# go to earth network
docker compose exec earth ash
oraid tx ibc-transfer transfer transfer channel-0 mars18vd8fpwxzck93qlwghaj6arh4p7c5n89plpqv0 10000000earth --from duc --chain-id Earth -y

# query mars network
curl http://mars:1317/cosmos/tx/v1beta1/txs?events=wasm.contract_address%3d%27mars18vd8fpwxzck93qlwghaj6arh4p7c5n89plpqv0%27&events=wasm.port%3d%27transfer%27

curl http://mars:1317/wasm/v1beta1/contract/mars174kgn5rtw4kf6f938wm7kwh70h2v4vcfknetgr/smart/eyJsaXN0X2FjY291bnRzIjp7fX0=

```
