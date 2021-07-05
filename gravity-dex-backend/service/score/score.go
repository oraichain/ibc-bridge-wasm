package score

import (
	"fmt"

	"github.com/b-harvest/gravity-dex-backend/schema"
	"github.com/b-harvest/gravity-dex-backend/service/price"
	"github.com/b-harvest/gravity-dex-backend/service/store"
	"github.com/b-harvest/gravity-dex-backend/util"
)

type Service struct {
	cfg Config
	ss  *store.Service
}

func NewService(cfg Config, ss *store.Service) *Service {
	return &Service{cfg: cfg, ss: ss}
}

func (s *Service) ActionScore(acc schema.Account) (float64, bool, error) {
	ds := acc.DepositStatus().NumDifferentPoolsByDate()
	ss := acc.SwapStatus().NumDifferentPoolsByDate()
	score := 0.0
	for _, k := range s.cfg.TradingDates {
		score += float64(util.MinInt(s.cfg.MaxActionScorePerDay, ds[k]))
		score += float64(util.MinInt(s.cfg.MaxActionScorePerDay, ss[k]))
	}
	score /= float64((2 * s.cfg.MaxActionScorePerDay) * len(s.cfg.TradingDates))
	score *= 100
	isValid := acc.DepositStatus().NumDifferentPools() >= 3 && acc.SwapStatus().NumDifferentPools() >= 3
	return score, isValid, nil
}

func (s *Service) TradingScore(acc schema.Account, priceTable price.Table) (float64, error) {
	if acc.Balance == nil {
		return 0, fmt.Errorf("missing account balance")
	}
	v := 0.0 // total usd value of the user's balances
	for _, c := range acc.Coins() {
		if c.Denom == "stake" { // TODO: do not use hardcoded stake coin denom
			continue
		}
		p, ok := priceTable[c.Denom]
		if !ok {
			return 0, fmt.Errorf("no price for denom %q", c.Denom)
		}
		v += p * float64(c.Amount)
	}
	return (v - s.cfg.InitialBalancesValue) / s.cfg.InitialBalancesValue * 100, nil
}

func (s *Service) TotalScore(actionScore, tradingScore float64) float64 {
	return actionScore*(1-s.cfg.TradingScoreRatio) + tradingScore*s.cfg.TradingScoreRatio
}
