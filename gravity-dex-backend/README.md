# gravity-dex-backend

## Prerequisites

Please install the latest version of:
- Go (1.x)
- MongoDB (4.x)
- Redis (6.x)

## Build & Install

```
$ go install ./...
```

This will build and install `gdex` binary in `$GOPATH/bin`.

## Usage

### Configuration

Transformer and Server requires a configuration file, `config.yml`, in current working directory.
All available configurations can be found in [here](./config/config.go)

### Transformer

Transformer keeps reading `transformer.block_data_dir` and synchronizes chain's state with the database.
Run it in background:
```
$ gdex transformer
```

### Server

Server is the API server.
It generates responses for each endpoints then caches those in background.
Run it with:
```
$ gdex server
```

## API Endpoints

### Score Board

#### Request

`GET /scoreboard?address=<string>`

`address` query parameter is optional.
If specified, `me` field is returned together in response.

#### Response

```
{
  "blockHeight" <int>,
  "me": { // optional, can be null.
    "ranking": <int>,
    "username": <string>,
    "address": <string>,
    "totalScore": <float>,
    "tradingScore": <float>,
    "actionScore": <float>
  }
  "accounts": [
    {
      "ranking": <int>
      "username": <string>,
      "address": <string>,
      "totalScore": <float>,
      "tradingScore": <float>,
      "actionScore": <float>
    },
    ...
  ],
  "updatedAt": <string>
}
```

If there is no account with matching address, then `me` field will contain `null`.

#### Errors

- `500 "no score board data found"`: There is no server cache of score board.

### Score Board - Search

#### Request

`GET /scoreboard/search?q=<string>`

`q` can be either a username or an address to search.

#### Response

```
{
  "blockHeight": <int>,
  "account": { // optional, can be null.
    "ranking": <int>
    "username": <string>,
    "address": <string>,
    "totalScore": <float>,
    "tradingScore": <float>,
    "actionScore": <float>
  },
  "updatedAt": <string>
}
```

If there is no account with matching address or username, then `account` field will contain `null`.

#### Errors

- `500 "no score board data found"`: There is no server cache of score board.

### Action Status

#### Request

`GET /actions?address=<string>`

#### Response

```
{
  "blockHeight": <int>,
  "account": { // optional, can be null.
    "deposit": {
      "numDifferentPools": <int>,
      "numDifferentPoolsToday": <int>,
      "maxNumDifferentPoolsToday": <int>
    },
    "swap": {
      "numDifferentPools": <int>,
      "numDifferentPoolsToday": <int>,
      "maxNumDifferentPoolsToday": <int>
    }
  },
  "updatedAt": <string>
}
```

If there is no account with matching address, then `account` field will contain `null`.

### Pools

#### Request

`GET /pools`

#### Response

```
{
  "blockHeight": <int>,
  "pools": [
    {
      "id": <uint>,
      "reserveCoins": [
        {
	  "denom": <string>,
	  "amount": <int>,
	  "globalPrice": <float>
	},
        {
	  "denom": <string>,
	  "amount": <int>,
	  "globalPrice": <float>
	}
      ],
      "poolCoin": {
        "demom": <string>,
	"amount": <int>,
	"globalPrice": <float>
      },
      "swapFeeValueSinceLastHour": <float>,
      "apy": <float>
    },
    ...
  ],
  "updatedAt": <string>
}
```

#### Errors

- `500 "no pool data found"`: There is no server cache of pools.

### Price Table

#### Request

`GET /prices`

#### Response

```
{
  "blockHeight": <int>,
  "prices": {
    <string>: <float>, // denom: globalPrice
    ...
  ],
  "updatedAt": <string>
}
```

#### Errors

- `500 "no price data found"`: There is no server cache of prices.

### Banner

#### Request

`GET /banner`

#### Response

```
{
  "banner": { // optional, can be null.
    "state": <string>, // "upcoming"|"started"
    "text": <string>,
    "url": <string>,
    "startsAt": <string>,
    "endsAt": <string>
  }
}
```

If there is no event upcoming or started, then `event` field will contain `null`.
