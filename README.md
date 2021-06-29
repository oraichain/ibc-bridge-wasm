## Installation

```bash
docker-compose up -d
docker-compose exec eath ash
 ./scripts/setup_genesis.sh
 oraid start --rpc.laddr tcp://0.0.0.0:26657
```

## mapping localhost

```bash
127.0.0.1 rpc.earth
127.0.0.1 lcd.earth
127.0.0.1 rpc.mars
127.0.0.1 lcd.mars
```

## send coin

```bash
node send.js --network earth --address earth1caqzkuacghaun8hled36v48etvj4g69mm9h295 --amount 10000
node send.js --network mars --address mars1caqzkuacghaun8hled36v48etvj4g69mak447x --amount 10000
# check balance
node send.js --network earth --address earth1caqzkuacghaun8hled36v48etvj4g69mm9h295
# or
yarn ibc-setup balances
```

## create ics20 channel

`yarn ibc-setup ics20 -v`

## start relayer

`yarn ibc-relayer start -v --poll 5`

## send cross-channel

```bash
# from earth to mars on channel
node send.js --network earth --address mars1k4klm6035ga0vjh7r7ez9ct3983tan4r449qlg --amount 100 --channel channel-0
# check balance on mars
node send.js --network mars
```
