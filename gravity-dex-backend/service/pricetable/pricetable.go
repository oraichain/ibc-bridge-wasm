package pricetable

import (
	"context"
	"fmt"
	"math"
	"math/rand"
	"strings"
	"time"

	"github.com/b-harvest/gravity-dex-backend/schema"
	"github.com/b-harvest/gravity-dex-backend/service/price"
	"github.com/b-harvest/gravity-dex-backend/util"
)

func init() {
	rand.Seed(time.Now().UnixNano())
}

type Service struct {
	cfg Config
	ps  price.Service
}

func NewService(cfg Config, ps price.Service) *Service {
	return &Service{cfg, ps}
}

func (s *Service) PriceTable(ctx context.Context, pools []schema.Pool) (price.Table, error) {
	t, err := s.ps.Prices(ctx, s.cfg.QueryableDenoms()...)
	if err != nil {
		return nil, fmt.Errorf("get prices: %w", err)
	}
	poolByPoolCoinDenom := make(map[string]*schema.Pool)
	for _, p := range pools {
		p := p
		poolByPoolCoinDenom[p.PoolCoinDenom] = &p
	}
	c := &Context{
		s.cfg.CoinDenoms,
		s.cfg.ManualPricesMap(),
		s.cfg.DenomMetadataMap(),
		t,
		poolByPoolCoinDenom,
	}
	denoms := s.cfg.AvailableDenoms()
	for denom := range poolByPoolCoinDenom {
		denoms = append(denoms, denom)
	}
	for _, denom := range denoms {
		if _, ok := t[denom]; !ok {
			_, err := c.Price(denom)
			if err != nil {
				return nil, fmt.Errorf("get price of denom %q: %w", denom, err)
			}
		}
	}
	return c.priceTable, nil
}

type Context struct {
	coinDenoms    []string
	manualPrices  map[string]ManualPrice
	denomMetadata map[string]DenomMetadata
	priceTable    price.Table
	pools         map[string]*schema.Pool
}

func (c *Context) IsNormalCoinDenom(denom string) bool {
	return util.StringInSlice(denom, c.coinDenoms)
}

func (c *Context) IsPoolCoinDenom(denom string) bool {
	if !strings.HasPrefix(denom, "pool") {
		return false
	}
	_, ok := c.pools[denom]
	return ok
}

func (c *Context) Price(denom string) (float64, error) {
	p, ok := c.priceTable[denom]
	if !ok {
		switch {
		case c.IsNormalCoinDenom(denom):
			mp, ok := c.manualPrices[denom]
			if !ok {
				return 0, fmt.Errorf("normal coin denom %q's price must be in price table", denom)
			}
			p = mp.MinPrice + rand.Float64()*(mp.MaxPrice-mp.MinPrice)
		case c.IsPoolCoinDenom(denom):
			pool := c.pools[denom]
			if pool.PoolCoinAmount() == 0 { // pool is inactive
				p = 0
				break
			}
			sum := 0.0
			for _, rc := range pool.ReserveCoins() {
				tp, err := c.Price(rc.Denom)
				if err != nil {
					return 0, err
				}
				sum += tp * float64(rc.Amount)
			}
			p = 1 / float64(pool.PoolCoinAmount()) * sum
		default:
			md, ok := c.denomMetadata[denom]
			if !ok {
				return 0, fmt.Errorf("unknown denom type: %s", denom)
			}
			tp, err := c.Price(md.Display)
			if err != nil {
				return 0, err
			}
			p = tp / math.Pow10(md.Exponent)
		}
		c.priceTable[denom] = p
	}
	return p, nil
}
