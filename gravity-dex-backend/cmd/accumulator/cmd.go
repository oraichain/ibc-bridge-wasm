package main

import (
	"context"
	"errors"
	"fmt"
	"log"
	"net/http"
	"os"
	"os/signal"
	"runtime"
	"sync"
	"syscall"
	"time"

	"github.com/gomodule/redigo/redis"
	"github.com/spf13/cobra"
)

var cfg AccumulatorConfig

func RootCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use: "accumulator",
	}
	cmd.PersistentFlags().StringVarP(&cfg.BlockDataDir, "dir", "d", "", "block data dir")
	cmd.PersistentFlags().IntVarP(&cfg.NumWorkers, "workers", "n", runtime.NumCPU(), "number of concurrent workers")
	cmd.PersistentFlags().DurationVarP(&cfg.TimeUnit, "unit", "u", 0, "time unit")
	cmd.PersistentFlags().StringSliceVarP(&cfg.WatchedAddresses, "watch", "w", nil, "watch addresses")
	_ = cmd.MarkFlagRequired("dir")
	cmd.AddCommand(ReplayCmd())
	cmd.AddCommand(ServerCmd())
	return cmd
}

func ReplayCmd() *cobra.Command {
	var startHeight, endHeight int64
	cmd := &cobra.Command{
		Use: "replay",
		RunE: func(cmd *cobra.Command, args []string) error {
			cmd.SilenceUsage = true

			acc, err := NewAccumulator(cfg, nil)
			if err != nil {
				return fmt.Errorf("new accumulator: %w", err)
			}

			if startHeight == 0 {
				return fmt.Errorf("start height must be greater than 0")
			}
			if endHeight == 0 {
				endHeight, err = acc.LatestBlockHeight()
				if err != nil {
					return fmt.Errorf("get latest block height: %w", err)
				}
			} else if endHeight <= startHeight {
				return fmt.Errorf("end height must be greater than %d", startHeight)
			}

			started := time.Now()
			data, err := acc.Accumulate(context.Background(), nil, startHeight, endHeight)
			if err != nil {
				return fmt.Errorf("accumulate: %w", err)
			}
			log.Printf("accumulated state in %s", time.Since(started))

			name := fmt.Sprintf("%d_%d.json", startHeight, endHeight)
			if err := WriteJSONFile(name, data); err != nil {
				return fmt.Errorf("write data file: %w", err)
			}
			log.Printf("wrote result to %s", name)

			return nil
		},
	}
	cmd.Flags().Int64VarP(&startHeight, "start", "s", 1, "replay start height")
	cmd.Flags().Int64VarP(&endHeight, "end", "e", 0, "replay end height")
	return cmd
}

func ServerCmd() *cobra.Command {
	var redisURL string
	var updateInterval time.Duration
	var bindAddr string
	cmd := &cobra.Command{
		Use: "server",
		RunE: func(cmd *cobra.Command, args []string) error {
			rp := &redis.Pool{
				Dial: func() (redis.Conn, error) {
					return redis.DialURL(redisURL)
				},
			}

			cm := NewCacheManager(rp, CacheKey)
			acc, err := NewAccumulator(cfg, cm)
			if err != nil {
				return fmt.Errorf("new accumulator: %w", err)
			}

			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()

			s := NewServer(cm)

			var wg sync.WaitGroup
			wg.Add(1)
			go func() {
				defer wg.Done()
				for {
					select {
					case <-ctx.Done():
						return
					default:
					}
					if err := acc.Run(ctx); err != nil {
						log.Printf("failed to run accumulator: %v", err)
					}
					select {
					case <-ctx.Done():
						return
					case <-time.After(updateInterval):
					}
				}
			}()

			wg.Add(1)
			go func() {
				defer wg.Done()
				log.Printf("server started on %s", bindAddr)
				if err := s.Start(bindAddr); err != nil && !errors.Is(err, http.ErrServerClosed) {
					log.Fatalf("failed run server: %v", err)
				}
			}()

			sigs := make(chan os.Signal, 1)
			signal.Notify(sigs, syscall.SIGINT, syscall.SIGTERM)
			<-sigs
			signal.Reset(syscall.SIGINT, syscall.SIGTERM)

			log.Printf("gracefully shutting down")
			cancel()
			if err := s.ShutdownWithTimeout(10 * time.Second); err != nil {
				log.Printf("failed to shutdown server: %v", err)
			}
			wg.Wait()

			return nil
		},
	}
	cmd.Flags().StringVarP(&redisURL, "redis", "r", "redis://redis", "redis url")
	cmd.Flags().DurationVarP(&updateInterval, "interval", "i", 30*time.Second, "update interval")
	cmd.Flags().StringVarP(&bindAddr, "bind", "b", "0.0.0.0:9000", "binding address")
	return cmd
}
