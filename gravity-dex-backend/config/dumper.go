package config

import (
	"fmt"

	"github.com/b-harvest/gravity-dex-backend/service/price"
	"github.com/b-harvest/gravity-dex-backend/service/pricetable"
	"github.com/b-harvest/gravity-dex-backend/service/score"
	"github.com/b-harvest/gravity-dex-backend/service/store"
)

type DumperConfig struct {
	DumpDir    string            `yaml:"dump_dir"`
	Store      store.Config      `yaml:"store"`
	Score      score.Config      `yaml:"score"`
	Price      price.Config      `yaml:"price"`
	PriceTable pricetable.Config `yaml:"pricetable"`
	MongoDB    MongoDBConfig     `yaml:"mongodb"`
}

var DefaultDumperConfig = DumperConfig{
	Store:      store.DefaultConfig,
	Score:      score.DefaultConfig,
	Price:      price.DefaultConfig,
	PriceTable: pricetable.DefaultConfig,
	MongoDB:    DefaultMongoDBConfig,
}

func (cfg DumperConfig) Validate() error {
	if cfg.DumpDir == "" {
		return fmt.Errorf("'dump_dir' is required")
	}
	if err := cfg.Store.Validate(); err != nil {
		return fmt.Errorf("validate 'store' field: %w", err)
	}
	if err := cfg.Score.Validate(); err != nil {
		return fmt.Errorf("validate 'score' field: %w", err)
	}
	if err := cfg.Price.Validate(); err != nil {
		return fmt.Errorf("validate 'price' field: %w", err)
	}
	if err := cfg.PriceTable.Validate(); err != nil {
		return fmt.Errorf("validate 'pricetable' field: %w", err)
	}
	return nil
}
