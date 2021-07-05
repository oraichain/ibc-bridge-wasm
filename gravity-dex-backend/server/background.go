package server

import (
	"context"
	"errors"
	"fmt"
	"time"

	"go.uber.org/zap"
	"golang.org/x/sync/errgroup"
)

func (s *Server) RunBackgroundUpdater(ctx context.Context) error {
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}
		s.logger.Debug("updating caches")
		if err := s.UpdateCaches(ctx); err != nil {
			if errors.Is(err, context.Canceled) {
				return err
			}
			s.logger.Error("failed to update caches", zap.Error(err))
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(s.cfg.CacheUpdateInterval):
		}
	}
}

func (s *Server) UpdateCaches(ctx context.Context) error {
	blockHeight, err := s.ss.LatestBlockHeight(ctx)
	if err != nil {
		return fmt.Errorf("get latest block height: %w", err)
	}
	pools, err := s.ss.Pools(ctx, blockHeight)
	if err != nil {
		return fmt.Errorf("get pools: %w", err)
	}
	t, err := s.pts.PriceTable(ctx, pools)
	if err != nil {
		return fmt.Errorf("get price table: %w", err)
	}
	eg, ctx2 := errgroup.WithContext(ctx)
	eg.Go(func() error {
		if err := s.UpdateAccountsCache(ctx2, blockHeight, t); err != nil {
			return fmt.Errorf("update accounts cache: %w", err)
		}
		return nil
	})
	eg.Go(func() error {
		if err := s.UpdatePoolsCache(ctx2, blockHeight, pools, t); err != nil {
			return fmt.Errorf("update pools cache: %w", err)
		}
		return nil
	})
	eg.Go(func() error {
		if err := s.UpdatePricesCache(ctx2, blockHeight, t); err != nil {
			return fmt.Errorf("update prices cache: %w", err)
		}
		return nil
	})
	return eg.Wait()
}
