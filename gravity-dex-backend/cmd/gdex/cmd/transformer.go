package cmd

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/spf13/cobra"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"
	"go.uber.org/zap"

	"github.com/b-harvest/gravity-dex-backend/config"
	"github.com/b-harvest/gravity-dex-backend/service/store"
	"github.com/b-harvest/gravity-dex-backend/transformer"
)

func TransformerCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "transformer",
		Short: "run transformer",
		RunE: func(cmd *cobra.Command, args []string) error {
			cmd.SilenceUsage = true

			cfg, err := config.Load("config.yml")
			if err != nil {
				return fmt.Errorf("load config: %w", err)
			}
			if err := cfg.Transformer.Validate(); err != nil {
				return fmt.Errorf("validate transformer config: %w", err)
			}

			logger, err := cfg.Transformer.Log.Build()
			if err != nil {
				return fmt.Errorf("build logger: %w", err)
			}
			defer logger.Sync()

			mc, err := mongo.Connect(context.Background(), options.Client().ApplyURI(cfg.Transformer.MongoDB.URI))
			if err != nil {
				return fmt.Errorf("connect mongodb: %w", err)
			}
			defer mc.Disconnect(context.Background())
			if err := mc.Ping(context.Background(), nil); err != nil {
				return fmt.Errorf("ping mongodb: %w", err)
			}

			ss := store.NewService(cfg.Transformer.Store, mc)
			names, err := ss.EnsureDBIndexes(context.Background())
			if err != nil {
				return fmt.Errorf("ensure db indexes: %w", err)
			}
			logger.Info("created db indexes", zap.Strings("names", names))

			t, err := transformer.New(cfg.Transformer, ss, logger)
			if err != nil {
				return fmt.Errorf("new transformer: %w", err)
			}

			logger.Info("started")

			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()

			var wg sync.WaitGroup
			wg.Add(1)
			go func() {
				defer wg.Done()
				for {
					select {
					case <-ctx.Done():
						return
					default:
						if err := t.Run(ctx); err != nil && !errors.Is(err, context.Canceled) {
							logger.Error("failed to run transformer", zap.Error(err))
						}
					}
					select {
					case <-ctx.Done():
						return
					case <-time.After(time.Second):
					}
				}
			}()

			quit := make(chan os.Signal, 1)
			signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
			<-quit

			logger.Info("gracefully shutting down")
			cancel()
			wg.Wait()
			return nil
		},
	}
	return cmd
}
