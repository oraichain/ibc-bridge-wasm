package store

import (
	"fmt"
)

type Config struct {
	DB                      string `yaml:"db"`
	CheckpointCollection    string `yaml:"checkpoint_collection"`
	AccountCollection       string `yaml:"account_collection"`
	AccountStatusCollection string `yaml:"account_status_collection"`
	PoolCollection          string `yaml:"pool_collection"`
	PoolStatusCollection    string `yaml:"pool_status_collection"`
	BalanceCollection       string `yaml:"balance_collection"`
	SupplyCollection        string `yaml:"supply_collection"`
	BannerCollection        string `yaml:"banner_collection"`
}

var DefaultConfig = Config{
	DB:                      "gdex",
	CheckpointCollection:    "checkpoint",
	AccountCollection:       "accounts",
	AccountStatusCollection: "accountStatuses",
	PoolCollection:          "pools",
	PoolStatusCollection:    "poolStatuses",
	BalanceCollection:       "balances",
	SupplyCollection:        "supplies",
	BannerCollection:        "banners",
}

func (cfg Config) Validate() error {
	if cfg.DB == "" {
		return fmt.Errorf("'db' is required")
	}
	// TODO: validate collection names
	return nil
}
