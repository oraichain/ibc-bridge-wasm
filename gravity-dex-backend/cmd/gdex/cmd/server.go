package cmd

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/gomodule/redigo/redis"
	"github.com/spf13/cobra"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"
	"go.uber.org/zap"
	"golang.org/x/sync/errgroup"

	"github.com/b-harvest/gravity-dex-backend/config"
	"github.com/b-harvest/gravity-dex-backend/server"
	"github.com/b-harvest/gravity-dex-backend/service/price"
	"github.com/b-harvest/gravity-dex-backend/service/pricetable"
	"github.com/b-harvest/gravity-dex-backend/service/score"
	"github.com/b-harvest/gravity-dex-backend/service/store"
)

func ServerCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "server",
		Short: "run web server",
		RunE: func(cmd *cobra.Command, args []string) error {
			cmd.SilenceUsage = true

			cfg, err := config.Load("config.yml")
			if err != nil {
				return fmt.Errorf("load config: %w", err)
			}
			if err := cfg.Server.Validate(); err != nil {
				return fmt.Errorf("validate server config: %w", err)
			}

			logger, err := cfg.Server.Log.Build()
			if err != nil {
				return fmt.Errorf("build logger: %w", err)
			}
			defer logger.Sync()

			mc, err := mongo.Connect(context.Background(), options.Client().ApplyURI(cfg.Server.MongoDB.URI))
			if err != nil {
				return fmt.Errorf("connect mongodb: %w", err)
			}
			defer mc.Disconnect(context.Background())
			if err := mc.Ping(context.Background(), nil); err != nil {
				return fmt.Errorf("ping mongodb: %w", err)
			}

			rp := &redis.Pool{
				Dial: func() (redis.Conn, error) {
					return redis.DialURL(cfg.Server.Redis.URI)
				},
			}
			defer rp.Close()
			conn := rp.Get()
			if _, err := conn.Do("PING"); err != nil {
				conn.Close()
				return fmt.Errorf("connect redis: %w", err)
			}
			conn.Close()

			ss := store.NewService(cfg.Server.Store, mc)
			ps, err := price.NewService(cfg.Server.Price)
			if err != nil {
				return fmt.Errorf("new price service: %w", err)
			}
			pts := pricetable.NewService(cfg.Server.PriceTable, ps)
			scs := score.NewService(cfg.Server.Score, ss)
			s := server.New(cfg.Server, ss, ps, pts, scs, rp, logger)

			names, err := ss.EnsureDBIndexes(context.Background())
			if err != nil {
				return fmt.Errorf("ensure db indexes: %w", err)
			}
			logger.Info("created db indexes", zap.Strings("names", names))
			logger.Info("starting server", zap.String("addr", cfg.Server.BindAddr))

			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			eg, ctx2 := errgroup.WithContext(ctx)
			eg.Go(func() error {
				if err := s.Start(cfg.Server.BindAddr); err != nil && !errors.Is(err, http.ErrServerClosed) {
					return fmt.Errorf("run server: %w", err)
				}
				return nil
			})
			eg.Go(func() error {
				if err := s.RunBackgroundUpdater(ctx2); err != nil {
					return fmt.Errorf("run background updater: %w", err)
				}
				return nil
			})

			quit := make(chan os.Signal, 1)
			signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
			<-quit

			logger.Info("gracefully shutting down")
			if err := s.ShutdownWithTimeout(10 * time.Second); err != nil {
				return fmt.Errorf("shutdown server: %w", err)
			}

			cancel()
			if err := eg.Wait(); !errors.Is(err, context.Canceled) {
				return err
			}
			return nil
		},
	}
	return cmd
}
