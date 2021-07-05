package config

import (
	"fmt"
	"time"

	"go.uber.org/zap"

	"github.com/b-harvest/gravity-dex-backend/service/store"
)

var DefaultTransformerConfig = TransformerConfig{
	BlockDataFilename:        "%08d/%d.json",
	BlockDataBucketSize:      10000,
	BlockDataWaitingInterval: time.Second,
	Store:                    store.DefaultConfig,
	MongoDB:                  DefaultMongoDBConfig,
	Log:                      zap.NewProductionConfig(),
}

type TransformerConfig struct {
	BlockDataDir             string        `yaml:"block_data_dir"`
	BlockDataFilename        string        `yaml:"block_data_filename"`
	BlockDataBucketSize      int           `yaml:"block_data_bucket_size"`
	BlockDataWaitingInterval time.Duration `yaml:"block_data_waiting_interval"`
	IgnoredAddresses         []string      `yaml:"ignored_addresses"`
	Store                    store.Config  `yaml:"store"`
	MongoDB                  MongoDBConfig `yaml:"mongodb"`
	Log                      zap.Config    `yaml:"log"`
}

func (cfg TransformerConfig) Validate() error {
	if cfg.BlockDataDir == "" {
		return fmt.Errorf("'block_data_dir' is required")
	}
	if err := cfg.Store.Validate(); err != nil {
		return fmt.Errorf("validate 'store' field: %w", err)
	}
	return nil
}

func (cfg TransformerConfig) IgnoredAddressesSet() map[string]struct{} {
	s := make(map[string]struct{})
	for _, a := range cfg.IgnoredAddresses {
		s[a] = struct{}{}
	}
	return s
}
