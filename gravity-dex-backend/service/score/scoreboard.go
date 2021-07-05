package score

import (
	"context"
	"fmt"
	"sort"
	"time"

	"github.com/b-harvest/gravity-dex-backend/schema"
	"github.com/b-harvest/gravity-dex-backend/service/price"
)

type Account struct {
	BlockHeight   int64
	Address       string
	Username      string
	Ranking       int
	TotalScore    float64
	ActionScore   float64
	TradingScore  float64
	IsValid       bool
	DepositStatus AccountActionStatus
	SwapStatus    AccountActionStatus
	UpdatedAt     time.Time
}

type AccountActionStatus struct {
	NumDifferentPools       int
	NumDifferentPoolsByDate map[string]int
}

func (s *Service) Scoreboard(ctx context.Context, blockHeight int64, priceTable price.Table) ([]Account, error) {
	now := time.Now()
	var accs []Account
	if err := s.ss.IterateAccounts(ctx, blockHeight, func(acc schema.Account) (stop bool, err error) {
		if acc.Username == "" {
			return false, nil
		}
		ts, err := s.TradingScore(acc, priceTable)
		if err != nil {
			return true, fmt.Errorf("calculate trading score for account %q: %w", acc.Address, err)
		}
		as, isValid, err := s.ActionScore(acc)
		if err != nil {
			return true, fmt.Errorf("calculate action score for account %q: %w", acc.Address, err)
		}
		accs = append(accs, Account{
			BlockHeight:  blockHeight,
			Address:      acc.Address,
			Username:     acc.Username,
			TotalScore:   ts*s.cfg.TradingScoreRatio + as*(1-s.cfg.TradingScoreRatio),
			ActionScore:  as,
			TradingScore: ts,
			IsValid:      isValid,
			DepositStatus: AccountActionStatus{
				NumDifferentPools:       acc.DepositStatus().NumDifferentPools(),
				NumDifferentPoolsByDate: acc.DepositStatus().NumDifferentPoolsByDate(),
			},
			SwapStatus: AccountActionStatus{
				NumDifferentPools:       acc.SwapStatus().NumDifferentPools(),
				NumDifferentPoolsByDate: acc.SwapStatus().NumDifferentPoolsByDate(),
			},
			UpdatedAt: now,
		})
		return false, nil
	}); err != nil {
		return nil, fmt.Errorf("iterate accounts: %w", err)
	}
	sort.SliceStable(accs, func(i, j int) bool {
		if accs[i].IsValid != accs[j].IsValid {
			return accs[i].IsValid
		}
		if accs[i].TotalScore != accs[j].TotalScore {
			return accs[i].TotalScore > accs[j].TotalScore
		}
		return accs[i].Address < accs[j].Address
	})
	for i := range accs {
		accs[i].Ranking = i + 1
	}
	return accs, nil
}
