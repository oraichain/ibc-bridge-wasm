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
docker-compose exec ibc ash
# default config, work the same as `yarn ibc-setup init --src earth --dest mars`
cp .ibc-setup/app.example.yaml .ibc-setup/app.yaml
yarn oraicli send --network earth --address earth1ya6nzd5jtzgmcn4vlueav4p3zdfhpvgngtwlpx --amount 60000
yarn oraicli send --network mars --address mars1ya6nzd5jtzgmcn4vlueav4p3zdfhpvgnwcvq65 --amount 60000
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
yarn oraicli ibc transfer --network earth --address mars1stnnv6qm9gnepjkvshh4aynrsrcr82zkyjfpph --amount 100 --channel channel-0
# check balance on mars
yarn oraicli account balance --network mars --address mars1stnnv6qm9gnepjkvshh4aynrsrcr82zkyjfpph
```

## deploy smart contract

```bash
yarn oraicli wasm deploy --file /root/contracts/ow721/artifacts/ow721.wasm  --label ow721 --network earth --input '{"minter":"earth16v74e2cmx2n0vsvw7dq5nzmwupgv9dqy8xpd07","name":"ow721","symbol":"NFT"}' --gas 3000000
```
