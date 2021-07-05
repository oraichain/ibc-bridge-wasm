package main

import (
	"fmt"
	"os"
	"sort"
	"strings"
	"sync"
	"time"

	sdk "github.com/cosmos/cosmos-sdk/types"
	jsoniter "github.com/json-iterator/go"
)

type Data struct {
	TimeBuckets map[string]*DataTimeBucket `json:"timeBuckets"`
	TimeUnit    time.Duration              `json:"timeUnit"`
	mux         sync.Mutex
}

func NewData() *Data {
	return &Data{
		TimeBuckets: make(map[string]*DataTimeBucket),
	}
}

func (data *Data) TimeBucketKey(t time.Time) string {
	return t.Truncate(data.TimeUnit).Format(TimeBucketKeyFormat)
}

func (data *Data) Sum(start, end time.Time) map[uint64]*PoolData {
	m := make(map[uint64]*PoolData)
	startT := start.UTC().Truncate(data.TimeUnit)
	endT := end.UTC().Truncate(data.TimeUnit)
	for !startT.After(endT) {
		bucketKey := data.TimeBucketKey(startT)
		b, ok := data.TimeBuckets[bucketKey]
		if ok {
			for poolID, p := range b.Pools {
				mp, ok := m[poolID]
				if !ok {
					mp = NewPoolData()
					m[poolID] = mp
				}
				mp.Add(p)
			}
		}
		startT = startT.Add(data.TimeUnit)
	}
	return m
}

func (data *Data) DepositCoins(bucketKey string, poolID uint64, coins sdk.Coins) {
	p := data.TimeBucket(bucketKey).Pool(poolID)
	p.NumDeposits++
	for _, coin := range coins {
		p.CoinsDeposited[coin.Denom] += coin.Amount.Int64()
	}
}

func (data *Data) WithdrawCoins(bucketKey string, poolID uint64, coins sdk.Coins) {
	p := data.TimeBucket(bucketKey).Pool(poolID)
	p.NumWithdrawals++
	for _, coin := range coins {
		p.CoinsWithdrawn[coin.Denom] += coin.Amount.Int64()
	}
}

func (data *Data) SwapCoin(bucketKey string, poolID uint64, offerCoin sdk.Coin, demandCoin sdk.Coin) {
	p := data.TimeBucket(bucketKey).Pool(poolID)
	if offerCoin.Denom < demandCoin.Denom {
		p.NumSwapsXToY++
	} else {
		p.NumSwapsYToX++
	}
	p.CoinsSwapped[offerCoin.Denom] += offerCoin.Amount.Int64()
	p.CoinsTransacted[offerCoin.Denom] += offerCoin.Amount.Int64()
	p.CoinsTransacted[demandCoin.Denom] += demandCoin.Amount.Int64()
}

func (data *Data) SwapCoinYToX(bucketKey string, poolID uint64, denom string, amount int64) {
	p := data.TimeBucket(bucketKey).Pool(poolID)
	p.NumSwapsYToX++
	p.CoinsSwapped[denom] += amount
}

type DataTimeBucket struct {
	Pools map[uint64]*PoolData `json:"pools"`
}

func NewDataTimeBucket() *DataTimeBucket {
	return &DataTimeBucket{
		Pools: make(map[uint64]*PoolData),
	}
}

func (data *Data) TimeBucket(key string) *DataTimeBucket {
	hs, ok := data.TimeBuckets[key]
	if !ok {
		hs = NewDataTimeBucket()
		data.TimeBuckets[key] = hs
	}
	return hs
}

type PoolData struct {
	NumDeposits     int   `json:"numDeposits"`
	CoinsDeposited  Coins `json:"coinsDeposited"`
	NumWithdrawals  int   `json:"numWithdrawals"`
	CoinsWithdrawn  Coins `json:"coinsWithdrawn"`
	NumSwapsXToY    int   `json:"numSwapsXToY"`
	NumSwapsYToX    int   `json:"numSwapsYToX"`
	CoinsSwapped    Coins `json:"coinsSwapped"`
	CoinsTransacted Coins `json:"coinsTransacted"`
}

func NewPoolData() *PoolData {
	return &PoolData{
		CoinsDeposited:  make(Coins),
		CoinsWithdrawn:  make(Coins),
		CoinsSwapped:    make(Coins),
		CoinsTransacted: make(Coins),
	}
}

func (b *DataTimeBucket) Pool(id uint64) *PoolData {
	p, ok := b.Pools[id]
	if !ok {
		p = NewPoolData()
		b.Pools[id] = p
	}
	return p
}

func (pd *PoolData) Add(other *PoolData) {
	pd.NumDeposits += other.NumDeposits
	pd.CoinsDeposited.Add(other.CoinsDeposited)
	pd.NumWithdrawals += other.NumWithdrawals
	pd.CoinsWithdrawn.Add(other.CoinsWithdrawn)
	pd.NumSwapsXToY += other.NumSwapsXToY
	pd.NumSwapsYToX += other.NumSwapsYToX
	pd.CoinsSwapped.Add(other.CoinsSwapped)
	pd.CoinsTransacted.Add(other.CoinsTransacted)
}

type Coins map[string]int64

func (cs Coins) String() string {
	var denoms []string
	for denom := range cs {
		denoms = append(denoms, denom)
	}
	sort.Strings(denoms)
	var ss []string
	for _, denom := range denoms {
		ss = append(ss, fmt.Sprintf("%d%s", cs[denom], denom))
	}
	return strings.Join(ss, ",")
}

func (cs Coins) Div(q int64) Coins {
	res := make(Coins)
	for denom, amount := range cs {
		res[denom] = amount / q
	}
	return res
}

func (cs Coins) Add(coins Coins) {
	for denom, amount := range coins {
		cs[denom] += amount
	}
}

func WriteJSONFile(name string, v interface{}) error {
	f, err := os.Create(name)
	if err != nil {
		return fmt.Errorf("create: %w", err)
	}
	defer f.Close()
	if err := jsoniter.NewEncoder(f).Encode(v); err != nil {
		return fmt.Errorf("encode: %w", err)
	}
	return nil
}
