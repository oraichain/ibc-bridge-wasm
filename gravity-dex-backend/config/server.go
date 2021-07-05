package config

import (
	"fmt"
	"time"

	"go.uber.org/zap"

	"github.com/b-harvest/gravity-dex-backend/service/price"
	"github.com/b-harvest/gravity-dex-backend/service/pricetable"
	"github.com/b-harvest/gravity-dex-backend/service/score"
	"github.com/b-harvest/gravity-dex-backend/service/store"
)

var DefaultServerConfig = ServerConfig{
	Debug:               false,
	BindAddr:            "0.0.0.0:8080",
	ScoreBoardSize:      100,
	CacheLoadTimeout:    10 * time.Second,
	CacheUpdateInterval: 5 * time.Second,
	AddressPrefix:       "cosmos1",
	Store:               store.DefaultConfig,
	Price:               price.DefaultConfig,
	PriceTable:          pricetable.DefaultConfig,
	Score:               score.DefaultConfig,
	MongoDB:             DefaultMongoDBConfig,
	Redis:               DefaultRedisConfig,
	Log:                 zap.NewProductionConfig(),
}

type ServerConfig struct {
	Debug               bool              `yaml:"debug"`
	BindAddr            string            `yaml:"bind_addr"`
	ScoreBoardSize      int               `yaml:"score_board_size"`
	CacheLoadTimeout    time.Duration     `yaml:"cache_load_timeout"`
	CacheUpdateInterval time.Duration     `yaml:"cache_update_interval"`
	AddressPrefix       string            `yaml:"address_prefix"`
	Store               store.Config      `yaml:"store"`
	Price               price.Config      `yaml:"price"`
	PriceTable          pricetable.Config `yaml:"pricetable"`
	Score               score.Config      `yaml:"score"`
	MongoDB             MongoDBConfig     `yaml:"mongodb"`
	Redis               RedisConfig       `yaml:"redis"`
	Log                 zap.Config        `yaml:"log"`
}

func (cfg ServerConfig) Validate() error {
	if err := cfg.Store.Validate(); err != nil {
		return fmt.Errorf("validate 'store' field: %w", err)
	}
	if err := cfg.Price.Validate(); err != nil {
		return fmt.Errorf("validate 'price' field: %w", err)
	}
	if err := cfg.PriceTable.Validate(); err != nil {
		return fmt.Errorf("validate 'pricetable' field: %w", err)
	}
	if err := cfg.Score.Validate(); err != nil {
		return fmt.Errorf("validate 'score' field: %w", err)
	}
	return nil
}
