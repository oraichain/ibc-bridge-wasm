package config

import (
	"os"

	"gopkg.in/yaml.v2"
)

var DefaultConfig = Config{
	Server:      DefaultServerConfig,
	Transformer: DefaultTransformerConfig,
	Dumper:      DefaultDumperConfig,
}

type Config struct {
	Server      ServerConfig      `yaml:"server"`
	Transformer TransformerConfig `yaml:"transformer"`
	Dumper      DumperConfig      `yaml:"dumper"`
}

func Load(path string) (Config, error) {
	f, err := os.Open(path)
	if err != nil {
		return Config{}, err
	}
	defer f.Close()
	cfg := DefaultConfig
	if err := yaml.NewDecoder(f).Decode(&cfg); err != nil {
		return Config{}, err
	}
	return cfg, nil
}

var DefaultMongoDBConfig = MongoDBConfig{
	URI: "mongodb://mongo",
}

type MongoDBConfig struct {
	URI string `yaml:"uri"`
}

var DefaultRedisConfig = RedisConfig{
	URI:                   "redis://redis",
	AccountCacheKeyPrefix: "gdex:account:",
	ScoreBoardCacheKey:    "gdex:scoreboard",
	PoolsCacheKey:         "gdex:pools",
	PricesCacheKey:        "gdex:prices",
}

type RedisConfig struct {
	URI                   string `yaml:"uri"`
	AccountCacheKeyPrefix string `yaml:"account_cache_key_prefix"`
	ScoreBoardCacheKey    string `yaml:"score_board_cache_key"`
	PoolsCacheKey         string `yaml:"pools_cache_key"`
	PricesCacheKey        string `yaml:"prices_cache_key"`
}
