package pricetable

import (
	"math"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/b-harvest/gravity-dex-backend/config"
	"github.com/b-harvest/gravity-dex-backend/schema"
	"github.com/b-harvest/gravity-dex-backend/service/price"
)

func TestContext_Price(t *testing.T) {
	ctx := &Context{
		coinDenoms: []string{"atom", "luna", "usd"},
		manualPrices: map[string]config.ManualPrice{
			"usd": {MinPrice: 1.0, MaxPrice: 1.0},
		},
		denomMetadata: map[string]config.DenomMetadata{
			"uusd":  {Display: "usd", Exponent: 6},
			"uatom": {Display: "atom", Exponent: 6},
			"uluna": {Display: "luna", Exponent: 6},
		},
		priceTable: price.Table{
			"atom": 20.0,
			"luna": 10.0,
		},
		pools: map[string]*schema.PoolStatus{
			"pool1": {
				ReserveCoins: []schema.Coin{
					{Denom: "uatom", Amount: 1000000},
					{Denom: "uusd", Amount: 20000000},
				},
				PoolCoin: schema.Coin{Denom: "pool1", Amount: 1000000},
			},
			"pool2": {
				ReserveCoins: []schema.Coin{
					{Denom: "uluna", Amount: 1000000},
					{Denom: "uusd", Amount: 10000000},
				},
				PoolCoin: schema.Coin{Denom: "pool2", Amount: 1000000},
			},
			"pool3": {
				ReserveCoins: []schema.Coin{
					{Denom: "uatom", Amount: 1000000},
					{Denom: "uluna", Amount: 2000000},
				},
				PoolCoin: schema.Coin{Denom: "pool3", Amount: 1000000},
			},
			"pool4": {
				ReserveCoins: []schema.Coin{
					{Denom: "pool1", Amount: 50000},
					{Denom: "pool2", Amount: 100000},
				},
				PoolCoin: schema.Coin{Denom: "pool4", Amount: 1000000},
			},
		},
	}
	for i, tc := range []struct {
		denom string
		price float64
	}{
		{"uatom", 0.00002},
		{"uluna", 0.00001},
		{"pool1", 0.00004},
		{"pool2", 0.00002},
		{"pool3", 0.00004},
		{"pool4", 0.00004},
	} {
		p, err := ctx.Price(tc.denom)
		require.NoError(t, err)
		assert.Truef(t, approxEqual(p, tc.price), "%f != %f, tc #%d", p, tc.price, i+1)
	}
}

func approxEqual(a, b float64) bool {
	return math.Abs(a-b) <= 0.001
}
