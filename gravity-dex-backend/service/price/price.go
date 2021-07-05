package price

import (
	"context"
	"fmt"
	"sync"

	"golang.org/x/sync/errgroup"
)

type Table map[string]float64

type Service interface {
	Prices(ctx context.Context, symbols ...string) (Table, error)
}

type service struct {
	cn  *CyberNodeService
	rnd *RandomOracleService
	fx  *FixerService
	cmc *CoinMarketCapService
}

func NewService(cfg Config) (Service, error) {
	rnd, err := NewRandomOracleService(cfg.RandomOracle.URL)
	if err != nil {
		return nil, fmt.Errorf("new random oracle service: %w", err)
	}
	fx, err := NewFixerService(cfg.Fixer.AccessKey, cfg.Fixer.UpdateInterval)
	if err != nil {
		return nil, fmt.Errorf("new fixer service: %w", err)
	}
	cmc, err := NewCoinMarketCapService(cfg.CoinMarketCap.APIKey, cfg.CoinMarketCap.UpdateInterval)
	if err != nil {
		return nil, fmt.Errorf("new coinmarketcap service: %w", err)
	}
	return &service{
		NewCyberNodeService(cfg.CyberNode.UpdateInterval),
		rnd,
		fx,
		cmc,
	}, nil
}

func (s *service) Prices(ctx context.Context, symbols ...string) (Table, error) {
	routes := make(map[string]Service)
	for _, srv := range []interface {
		Service
		Symbols() []string
	}{
		s.rnd,
		s.cn,
		s.fx,
	} {
		for _, symbol := range srv.Symbols() {
			routes[symbol] = srv
		}
	}
	m := make(map[Service][]string)
	for _, symbol := range symbols {
		if srv, ok := routes[symbol]; ok {
			m[srv] = append(m[srv], symbol)
		} else {
			m[s.cmc] = append(m[s.cmc], symbol)
		}
	}
	res := make(Table)
	var mux sync.Mutex
	eg, ctx2 := errgroup.WithContext(ctx)
	for srv, ss := range m {
		srv := srv
		ss := ss
		eg.Go(func() error {
			t, err := srv.Prices(ctx2, ss...)
			if err != nil {
				return err
			}
			mux.Lock()
			defer mux.Unlock()
			for k, v := range t {
				res[k] = v
			}
			return nil
		})
	}
	if err := eg.Wait(); err != nil {
		return nil, err
	}
	return res, nil
}
