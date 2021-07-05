package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"time"

	jsoniter "github.com/json-iterator/go"
	liquiditytypes "github.com/tendermint/liquidity/x/liquidity/types"
	"golang.org/x/sync/errgroup"
)

const TimeBucketKeyFormat = "2006-01-02T15:04:05"

type AccumulatorConfig struct {
	BlockDataDir     string
	NumWorkers       int
	TimeUnit         time.Duration
	WatchedAddresses []string
}

type Accumulator struct {
	cfg              AccumulatorConfig
	cm               *CacheManager
	watchedAddresses map[string]struct{}
}

func NewAccumulator(cfg AccumulatorConfig, cm *CacheManager) (*Accumulator, error) {
	if cfg.NumWorkers == 0 {
		cfg.NumWorkers = runtime.NumCPU()
	}
	if cfg.TimeUnit == 0 {
		cfg.TimeUnit = time.Hour
	}
	if _, err := os.Stat(cfg.BlockDataDir); err != nil {
		return nil, fmt.Errorf("check block data dir: %w", err)
	}
	acc := &Accumulator{
		cfg:              cfg,
		cm:               cm,
		watchedAddresses: make(map[string]struct{}),
	}
	for _, addr := range cfg.WatchedAddresses {
		acc.watchedAddresses[addr] = struct{}{}
	}
	return acc, nil
}

func (acc *Accumulator) LatestBlockBucket() (int64, error) {
	es, err := os.ReadDir(acc.cfg.BlockDataDir)
	if err != nil {
		return 0, fmt.Errorf("read dir: %w", err)
	}
	var buckets []int64
	for _, e := range es {
		if !e.IsDir() {
			continue
		}
		var n int64
		if _, err := fmt.Sscanf(e.Name(), "%08d", &n); err != nil {
			continue
		}
		buckets = append(buckets, n)
	}
	if len(buckets) == 0 {
		return 0, fmt.Errorf("no buckets")
	}
	sort.Slice(buckets, func(i, j int) bool {
		return buckets[i] > buckets[j]
	})
	return buckets[0], nil
}

func (acc *Accumulator) LatestBlockHeight() (int64, error) {
	bucket, err := acc.LatestBlockBucket()
	if err != nil {
		return 0, fmt.Errorf("get latest block bucket: %w", err)
	}
	es, err := os.ReadDir(acc.BlockDataBucketDir(bucket))
	if err != nil {
		return 0, fmt.Errorf("read dir: %w", err)
	}
	var heights []int64
	for _, e := range es {
		if e.IsDir() {
			continue
		}
		var height int64
		if _, err := fmt.Sscanf(e.Name(), "%d.json", &height); err != nil {
			continue
		}
		heights = append(heights, height)
	}
	if len(heights) == 0 {
		return 0, fmt.Errorf("no blocks")
	}
	sort.Slice(heights, func(i, j int) bool {
		return heights[i] > heights[j]
	})
	return heights[0], nil
}

func (acc *Accumulator) BlockDataBucketDir(bucket int64) string {
	return filepath.Join(acc.cfg.BlockDataDir, fmt.Sprintf("%08d", bucket))
}

func (acc *Accumulator) BlockDataFilename(height int64) string {
	bs := int64(10000)
	p := height / bs * bs
	return filepath.Join(acc.cfg.BlockDataDir, fmt.Sprintf("%08d", p), fmt.Sprintf("%d.json", height))
}

func (acc *Accumulator) ReadBlockData(height int64) (*BlockData, error) {
	f, err := os.Open(acc.BlockDataFilename(height))
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var blockData BlockData
	if err := jsoniter.NewDecoder(f).Decode(&blockData); err != nil {
		return nil, err
	}
	if blockData.Header.Height != height {
		return nil, fmt.Errorf("wrong block height; expected %d, got %d", height, blockData.Header.Height)
	}
	return &blockData, nil
}

func (acc *Accumulator) UpdateData(ctx context.Context, blockData *BlockData, data *Data) error {
	data.mux.Lock()
	defer data.mux.Unlock()
	t := blockData.Header.Time
	height := blockData.Header.Height
	bucketKey := acc.TimeBucketKey(t)
	poolByID := blockData.PoolByID()
	for _, evt := range blockData.Events {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}
		switch evt.Type {
		case liquiditytypes.EventTypeDepositToPool:
			evt, err := NewDepositEvent(evt)
			if err != nil {
				return fmt.Errorf("extract deposit event: %w", err)
			}
			if _, ok := acc.watchedAddresses[evt.DepositorAddress]; ok {
				pool, ok := poolByID[evt.PoolID]
				if !ok {
					return fmt.Errorf("pool %d nout found", evt.PoolID)
				}
				fmt.Printf("[%d/%s] %s deposits %v to %s/%s pool\n",
					height, t.Format(time.RFC3339), evt.DepositorAddress, evt.AcceptedCoins,
					pool.ReserveCoinDenoms[0], pool.ReserveCoinDenoms[1])
			}
			data.DepositCoins(bucketKey, evt.PoolID, evt.AcceptedCoins)
		case liquiditytypes.EventTypeWithdrawFromPool:
			evt, err := NewWithdrawEvent(evt)
			if err != nil {
				return fmt.Errorf("extract withdraw event: %w", err)
			}
			if _, ok := acc.watchedAddresses[evt.WithdrawerAddress]; ok {
				pool, ok := poolByID[evt.PoolID]
				if !ok {
					return fmt.Errorf("pool %d nout found", evt.PoolID)
				}
				fmt.Printf("[%d/%s] %s withdraws %v to %s/%s pool\n",
					height, t.Format(time.RFC3339), evt.WithdrawerAddress, evt.WithdrawnCoins,
					pool.ReserveCoinDenoms[0], pool.ReserveCoinDenoms[1])
			}
			data.WithdrawCoins(bucketKey, evt.PoolID, evt.WithdrawnCoins)
		case liquiditytypes.EventTypeSwapTransacted:
			evt, err := NewSwapEvent(evt, poolByID)
			if err != nil {
				return fmt.Errorf("extract swap event: %w", err)
			}
			pool, ok := poolByID[evt.PoolID]
			if !ok {
				return fmt.Errorf("pool %d not found", evt.PoolID)
			}
			if _, ok := acc.watchedAddresses[evt.SwapRequesterAddress]; ok {
				fmt.Printf("[%d/%s] %s swaps %s to %s in %s/%s pool\n",
					height, t.Format(time.RFC3339), evt.SwapRequesterAddress, evt.ExchangedOfferCoin,
					evt.ExchangedDemandCoin, pool.ReserveCoinDenoms[0], pool.ReserveCoinDenoms[1])
			}
			data.SwapCoin(bucketKey, evt.PoolID, evt.ExchangedOfferCoin, evt.ExchangedDemandCoin)
		}
	}
	return nil
}

func (acc *Accumulator) Accumulate(ctx context.Context, data *Data, startHeight, endHeight int64) (*Data, error) {
	if data == nil {
		data = NewData()
	}
	jobs := make(chan int64, endHeight-startHeight)

	worker := func(ctx context.Context) error {
		for {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case height, ok := <-jobs:
				if !ok {
					return nil
				}
				blockData, err := acc.ReadBlockData(height)
				if err != nil {
					return err
				}
				if err := acc.UpdateData(ctx, blockData, data); err != nil {
					return err
				}
			}
		}
	}

	eg, ctx2 := errgroup.WithContext(ctx)
	for i := 0; i < acc.cfg.NumWorkers; i++ {
		eg.Go(func() error {
			return worker(ctx2)
		})
	}

	for height := startHeight; height <= endHeight; height++ {
		jobs <- height
	}
	close(jobs)

	if err := eg.Wait(); err != nil {
		return nil, err
	}

	data.TimeUnit = acc.cfg.TimeUnit

	return data, nil
}

func (acc *Accumulator) Run(ctx context.Context) error {
	c, err := acc.cm.Get(ctx)
	if err != nil {
		return fmt.Errorf("get cache: %w", err)
	}
	var data *Data
	blockHeight := int64(1)
	if c != nil {
		data = c.Data
		blockHeight = c.BlockHeight
	}

	if blockHeight > 1 {
		log.Printf("last cached block height: %v", c.BlockHeight)
	} else {
		log.Printf("no cache found")
	}

	h, err := acc.LatestBlockHeight()
	if err != nil {
		return fmt.Errorf("get latest block height: %w", err)
	}

	if blockHeight >= h {
		log.Printf("the state is up to date")
	} else {
		log.Printf("accumulating from %d to %d", blockHeight, h)

		started := time.Now()
		data, err = acc.Accumulate(ctx, data, blockHeight, h)
		if err != nil {
			return fmt.Errorf("run accumulator: %w", err)
		}
		log.Printf("accumulated state in %s", time.Since(started))

		if err := acc.cm.Set(ctx, &Cache{
			BlockHeight: h,
			Data:        data,
		}); err != nil {
			return fmt.Errorf("set cache: %w", err)
		}
		log.Printf("saved cache")
	}

	return nil
}

func (acc *Accumulator) TimeBucketKey(t time.Time) string {
	return t.Truncate(acc.cfg.TimeUnit).Format(TimeBucketKeyFormat)
}
