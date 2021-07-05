package price

import (
	"fmt"
	"time"
)

type Config struct {
	CoinMarketCap CoinMarketCapConfig `yaml:"coinmarketcap"`
	CyberNode     CyberNodeConfig     `yaml:"cybernode"`
	Fixer         FixerConfig         `yaml:"fixer"`
	RandomOracle  RandomOracleConfig  `yaml:"random_oracle"`
}

var DefaultConfig = Config{
	CoinMarketCap: CoinMarketCapConfig{
		UpdateInterval: time.Minute,
	},
	CyberNode: CyberNodeConfig{
		UpdateInterval: time.Minute,
	},
	Fixer: FixerConfig{
		UpdateInterval: time.Minute,
	},
}

type CoinMarketCapConfig struct {
	APIKey         string        `yaml:"api_key"`
	UpdateInterval time.Duration `yaml:"update_interval"`
}

type CyberNodeConfig struct {
	UpdateInterval time.Duration `yaml:"update_interval"`
}

type FixerConfig struct {
	AccessKey      string        `yaml:"access_key"`
	UpdateInterval time.Duration `yaml:"update_interval"`
}

type RandomOracleConfig struct {
	URL string `yaml:"url"`
}

func (cfg Config) Validate() error {
	if cfg.CoinMarketCap.APIKey == "" {
		return fmt.Errorf("'coinmarketcap.api_key' is required")
	}
	if cfg.Fixer.AccessKey == "" {
		return fmt.Errorf("'fixer.access_key' is required")
	}
	if cfg.RandomOracle.URL == "" {
		return fmt.Errorf("'random_oracle.url' is required")
	}
	return nil
}
