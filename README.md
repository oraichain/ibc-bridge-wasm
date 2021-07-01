## Installation

```bash
docker compose up -d
```

## mapping localhost for checking lcd and rpc api

```bash
127.0.0.1 rpc.earth
127.0.0.1 lcd.earth
127.0.0.1 rpc.mars
127.0.0.1 lcd.mars
```

## send coin

```bash
docker compose exec ibc ash
# default config, work the same as `yarn ibc-setup init --src earth --dest mars`
cp .ibc-setup/app.example.yaml .ibc-setup/app.yaml
yarn oraicli send --network earth --address earth1ya6nzd5jtzgmcn4vlueav4p3zdfhpvgngtwlpx --amount 6000000
yarn oraicli send --network mars --address mars1ya6nzd5jtzgmcn4vlueav4p3zdfhpvgnwcvq65 --amount 6000000
# check balance
yarn ibc-setup balances
```

## create ics20 channel

`yarn ibc-setup ics20 -v`

## start relayer

`yarn ibc-relayer start -v --poll 5`

## send cross-channel

```bash
# from earth to mars on channel
yarn oraicli ibc transfer --network earth --address mars1ya6nzd5jtzgmcn4vlueav4p3zdfhpvgnwcvq65 --amount 100 --channel channel-0
# check balance on mars
yarn oraicli account balance --network mars --address mars1ya6nzd5jtzgmcn4vlueav4p3zdfhpvgnwcvq65
```

## deploy smart contract

```bash
# deploy marketplace contract on mars blockchain
yarn oraicli wasm deploy --file /root/contracts/marketplace/artifacts/marketplace.wasm --label marketplace --network mars --input '{"name": "nft market"}' --gas 3000000

yarn oraicli wasm deploy --file /root/contracts/ow721/artifacts/ow721.wasm --label ow721 --network mars --input '{"minter":marketplace_contract,"name":"NFT Collection","symbol":"NFT"}' --gas 3000000
```
