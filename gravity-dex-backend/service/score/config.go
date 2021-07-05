package score

import (
	"fmt"
)

type Config struct {
	TradingScoreRatio    float64  `yaml:"trading_score_ratio"`
	InitialBalancesValue float64  `yaml:"initial_balances_value"`
	MaxActionScorePerDay int      `yaml:"max_action_score_per_day"`
	TradingDates         []string `yaml:"trading_dates"`
}

var DefaultConfig = Config{
	TradingScoreRatio:    0.9,
	InitialBalancesValue: 40000,
	MaxActionScorePerDay: 3,
	TradingDates: []string{
		"2021-05-04",
		"2021-05-05",
		"2021-05-06",
		"2021-05-07",
		"2021-05-08",
		"2021-05-09",
		"2021-05-10",
	},
}

func (cfg Config) Validate() error {
	if len(cfg.TradingDates) == 0 {
		return fmt.Errorf("'trading_dates' is empty")
	}
	if cfg.InitialBalancesValue <= 0 {
		return fmt.Errorf("'initial_balances_value' must be positive")
	}
	if cfg.TradingScoreRatio < 0 || cfg.TradingScoreRatio > 1 {
		return fmt.Errorf("'trading_score_ratio' must be between 0~1")
	}
	return nil
}
