package schema

import (
	"time"

	sdk "github.com/cosmos/cosmos-sdk/types"
)

const (
	CheckpointBlockHeightKey = "blockHeight"
	CheckpointTimestampKey   = "timestamp"
)

type Checkpoint struct {
	BlockHeight int64     `bson:"blockHeight"`
	Timestamp   time.Time `bson:"timestamp"`
}

const (
	AccountAddressKey   = "address"
	AccountUsernameKey  = "username"
	AccountIsBlockedKey = "isBlocked"
	AccountBlockedAtKey = "blockedAt"
	AccountCreatedAtKey = "createdAt"
	AccountStatusKey    = "status"
	AccountBalanceKey   = "balance"
)

type Account struct {
	Address   string     `bson:"address"`
	Username  string     `bson:"username"`
	IsBlocked bool       `bson:"isBlocked"`
	BlockedAt *time.Time `bson:"blockedAt,omitempty"`
	CreatedAt time.Time  `bson:"createdAt"`

	Status  *AccountStatus `bson:"status"`
	Balance *Balance       `bson:"balance"`
}

func (acc Account) DepositStatus() AccountActionStatus {
	if acc.Status != nil {
		return acc.Status.Deposits
	}
	return AccountActionStatus{}
}

func (acc Account) SwapStatus() AccountActionStatus {
	if acc.Status != nil {
		return acc.Status.Swaps
	}
	return AccountActionStatus{}
}

func (acc Account) Coins() []Coin {
	if acc.Balance != nil {
		return acc.Balance.Coins
	}
	return nil
}

const (
	AccountStatusBlockHeightKey = "blockHeight"
	AccountStatusAddressKey     = "address"
	AccountStatusDepositsKey    = "deposits"
	AccountStatusSwapsKey       = "swaps"
)

type AccountStatus struct {
	BlockHeight int64               `bson:"blockHeight"`
	Address     string              `bson:"address"`
	Deposits    AccountActionStatus `bson:"deposits"`
	Swaps       AccountActionStatus `bson:"swaps"`
}

type AccountActionStatus struct {
	CountByPoolID       CountByPoolID            `bson:"countByPoolID"`
	CountByPoolIDByDate map[string]CountByPoolID `bson:"countByPoolIDByDate"`
}

type CountByPoolID map[uint64]int

func NewAccountActionStatus() AccountActionStatus {
	return AccountActionStatus{
		CountByPoolID:       make(CountByPoolID),
		CountByPoolIDByDate: make(map[string]CountByPoolID),
	}
}

func MergeAccountActionStatuses(ss ...AccountActionStatus) AccountActionStatus {
	s := NewAccountActionStatus()
	for _, s2 := range ss {
		for date, c := range s2.CountByPoolIDByDate {
			for id, c2 := range c {
				s.IncreaseCount(id, date, c2)
			}
		}
	}
	return s
}

func (s AccountActionStatus) NumDifferentPools() int {
	return len(s.CountByPoolID)
}

func (s AccountActionStatus) NumDifferentPoolsByDate() map[string]int {
	m := make(map[string]int)
	for date, c := range s.CountByPoolIDByDate {
		m[date] = len(c)
	}
	return m
}

func (s *AccountActionStatus) IncreaseCount(poolID uint64, date string, amount int) {
	s.CountByPoolID[poolID] += amount
	c, ok := s.CountByPoolIDByDate[date]
	if !ok {
		c = make(CountByPoolID)
		s.CountByPoolIDByDate[date] = c
	}
	c[poolID] += amount
}

const (
	BalanceBlockHeightKey = "blockHeight"
	BalanceAddressKey     = "address"
	BalanceCoinsKey       = "coins"
)

type Balance struct {
	BlockHeight int64  `bson:"blockHeight"`
	Address     string `bson:"address"`
	Coins       []Coin `bson:"coins"`
}

type Coin struct {
	Denom  string `bson:"denom"`
	Amount int64  `bson:"amount"`
}

func CoinFromSDK(coin sdk.Coin) Coin {
	return Coin{Denom: coin.Denom, Amount: coin.Amount.Int64()}
}

func CoinsFromSDK(coins sdk.Coins) []Coin {
	var cs []Coin
	for _, c := range coins {
		cs = append(cs, CoinFromSDK(c))
	}
	return cs
}

const (
	SupplyBlockHeightKey = "blockHeight"
	SupplyDenomKey       = "denom"
	SupplyAmountKey      = "amount"
)

type Supply struct {
	BlockHeight int64 `bson:"blockHeight"`
	Coin        `bson:",inline"`
}

const (
	PoolIDKey                    = "id"
	PoolReserveAccountAddressKey = "reserveAccountAddress"
	PoolReserveCoinDenomsKey     = "reserveCoinDenoms"
	PoolPoolCoinDenomKey         = "poolCoinDenom"
	PoolStatusKey                = "status"
	PoolReserveAccountBalanceKey = "reserveAccountBalance"
	PoolPoolCoinSupplyKey        = "poolCoinSupply"
)

type Pool struct {
	ID                    uint64   `bson:"id"`
	ReserveAccountAddress string   `bson:"reserveAccountAddress"`
	ReserveCoinDenoms     []string `bson:"reserveCoinDenoms"`
	PoolCoinDenom         string   `bson:"poolCoinDenom"`

	Status                *PoolStatus `bson:"status"`
	ReserveAccountBalance *Balance    `bson:"reserveAccountBalance"`
	PoolCoinSupply        *Supply     `bson:"poolCoinSupply"`
}

func (p Pool) SwapFeeVolumes() Volumes {
	if p.Status != nil {
		return p.Status.SwapFeeVolumes
	}
	return Volumes{}
}

func (p Pool) ReserveCoins() []Coin {
	var cs []Coin
	if p.ReserveAccountBalance != nil {
		cm := make(map[string]Coin)
		for _, c := range p.ReserveAccountBalance.Coins {
			cm[c.Denom] = c
		}
		for _, denom := range p.ReserveCoinDenoms {
			cs = append(cs, cm[denom])
		}
	}
	return cs
}

func (p Pool) PoolCoinAmount() int64 {
	if p.PoolCoinSupply != nil {
		return p.PoolCoinSupply.Amount
	}
	return 0
}

const (
	PoolStatusBlockHeightKey    = "blockHeight"
	PoolStatusIDKey             = "id"
	PoolStatusSwapFeeVolumesKey = "swapFeeVolumes"
)

type PoolStatus struct {
	BlockHeight    int64   `bson:"blockHeight"`
	ID             uint64  `bson:"id"`
	SwapFeeVolumes Volumes `bson:"swapFeeVolumes"`
}

const VolumeTimeUnit = time.Minute

type Volumes map[int64]CoinMap

func MergeVolumes(vs ...Volumes) Volumes {
	v := make(Volumes)
	for _, v2 := range vs {
		for t, c2 := range v2 {
			t = time.Unix(t, 0).Truncate(VolumeTimeUnit).Unix()
			c, ok := v[t]
			if !ok {
				c = make(CoinMap)
				v[t] = c
			}
			c.Add(c2)
		}
	}
	return v
}

func (v Volumes) TotalCoins() CoinMap {
	c := make(CoinMap)
	for _, c2 := range v {
		c.Add(c2)
	}
	return c
}

func (v Volumes) AddCoins(now time.Time, c2 CoinMap) {
	t := now.Truncate(VolumeTimeUnit).Unix()
	c, ok := v[t]
	if !ok {
		c = make(CoinMap)
		v[t] = c
	}
	c.Add(c2)
}

func (v Volumes) RemoveOutdated(past time.Time) {
	p := past.UTC().Unix()
	for t := range v {
		if t < p {
			delete(v, t)
		}
	}
}

type CoinMap map[string]int64

func (c CoinMap) Add(c2 CoinMap) {
	for denom, amount := range c2 {
		c[denom] += amount
	}
}

const (
	BannerVisibleAtKey = "visibleAt"
	BannerStartsAtKey  = "startsAt"
	BannerEndsAtKey    = "endsAt"
)

type Banner struct {
	UpcomingText string    `bson:"upcomingText"`
	Text         string    `bson:"text"`
	URL          string    `bson:"url"`
	VisibleAt    time.Time `bson:"visibleAt"`
	StartsAt     time.Time `bson:"startsAt"`
	EndsAt       time.Time `bson:"endsAt"`
}
