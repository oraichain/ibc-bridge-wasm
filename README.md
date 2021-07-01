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

## using wasm global

### Sell CW721 Token

Puts an NFT token up for sale.

> :warning: The seller needs to be the owner of the token to be able to sell it.

```js
// Execute mint to create new NFT Token with fake_receiver_addr account
await Wasm.execute(
  'marketplace',
  JSON.stringify({
    mint_nft: {
      contract: 'nft',
      msg: btoa(
        JSON.stringify({
          mint: {
            description: 'nft desc',
            image:
              'https://ipfs.io/ipfs/QmWCp5t1TLsLQyjDFa87ZAp72zYqmC7L2DsNjFdpH8bBoz',
            name: 'nft rare',
            owner: 'fake_receiver_addr',
            token_id: '123456'
          }
        })
      )
    }
  }),
  'fake_receiver_addr'
);

// Execute send_nft action to put token up for sale for specified list_price on the marketplace
await Wasm.execute(
  'ow721',
  JSON.stringify({
    send_nft: {
      contract: 'marketplace',
      msg: btoa(
        JSON.stringify({
          price: '50'
        })
      ),
      token_id: '123456'
    }
  }),
  'fake_receiver_addr'
);
```

### Query Offerings

Retrieves a list of all currently listed offerings.

```js
await Wasm.query(
  'marketplace',
  JSON.stringify({
    get_offerings: { limit: 10, offset: '0' }
  })
);
```

### Withdraw CW721 Token Offering

Withdraws an NFT token offering from the global offerings list and returns the NFT token back to its owner.

> :warning: Only the token's owner/seller can withdraw the offering. This will only work after having used `sell_nft` on a token.

```js
// Execute withdraw_nft action to withdraw the token with the specified offering_id from the marketplace
await Wasm.execute(
  'marketplace',
  JSON.stringify({
    withdraw_nft: {
      offering_id: '1'
    }
  })
);
```

### Buy CW721 Token

Buys an NFT token, transferring funds to the seller and the token to the buyer.

> :warning: This will only work after having used `sell_nft` on a token.

```js
// Execute send action to buy token with the specified offering_id from the marketplace
// denom: mars or ibc/sha256(transfer/channel-0/earth)
await Wasm.execute(
  'marketplace',
  JSON.stringify({
    buy_nft: {
      offering_id: 50
    }
  }),
  { funds: [{ denom, amount }] }
);
```

### Check Owner of NFT

```js
await Wasm.query(
  'ow721',
  JSON.stringify({
    owner_of: {
      token_id: '123456'
    }
  })
);
```
