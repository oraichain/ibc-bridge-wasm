package schema

import "time"

type AccountCache struct {
	BlockHeight   int64                    `json:"H"`
	Address       string                   `json:"A"`
	Username      string                   `json:"U"`
	Ranking       int                      `json:"R"`
	TotalScore    float64                  `json:"S"`
	ActionScore   float64                  `json:"AS"`
	TradingScore  float64                  `json:"T"`
	IsValid       bool                     `json:"V"`
	DepositStatus AccountCacheActionStatus `json:"D"`
	SwapStatus    AccountCacheActionStatus `json:"SS"`
	UpdatedAt     time.Time                `json:"UA"`
}

type AccountCacheActionStatus struct {
	NumDifferentPools       int            `json:"N"`
	NumDifferentPoolsByDate map[string]int `json:"B"`
}

type ScoreBoardCache struct {
	BlockHeight int64          `json:"H"`
	Accounts    []AccountCache `json:"A"`
	UpdatedAt   time.Time      `json:"U"`
}

type PoolsCache struct {
	BlockHeight      int64            `json:"blockHeight"`
	Pools            []PoolsCachePool `json:"pools"`
	TotalValueLocked float64          `json:"totalValueLocked"`
	UpdatedAt        time.Time        `json:"updatedAt"`
}

type PoolsCachePool struct {
	ID                        uint64           `json:"id"`
	ReserveCoins              []PoolsCacheCoin `json:"reserveCoins"`
	PoolCoin                  PoolsCacheCoin   `json:"poolCoin"`
	SwapFeeValueSinceLastHour float64          `json:"swapFeeValueSinceLastHour"`
	APY                       float64          `json:"apy"`
}

type PoolsCacheCoin struct {
	Denom       string  `json:"denom"`
	Amount      int64   `json:"amount"`
	GlobalPrice float64 `json:"globalPrice"`
}

type PricesCache struct {
	BlockHeight int64              `json:"blockHeight"`
	Prices      map[string]float64 `json:"prices"`
	UpdatedAt   time.Time          `json:"updatedAt"`
}
