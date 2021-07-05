package main

import (
	"bytes"
	"context"
	"encoding/gob"
	"errors"
	"fmt"

	"github.com/gomodule/redigo/redis"
)

var CacheKey = "gdex-accumulator:cache"

type Cache struct {
	BlockHeight int64
	Data        *Data
}

type CacheManager struct {
	rp  *redis.Pool
	key string
}

func NewCacheManager(rp *redis.Pool, key string) *CacheManager {
	return &CacheManager{rp: rp, key: key}
}

func (cm *CacheManager) Get(ctx context.Context) (*Cache, error) {
	conn, err := cm.rp.GetContext(ctx)
	if err != nil {
		return nil, fmt.Errorf("get redis conn: %w", err)
	}
	defer conn.Close()
	b, err := redis.Bytes(conn.Do("GET", cm.key))
	if err != nil {
		if errors.Is(err, redis.ErrNil) {
			return nil, nil
		}
		return nil, err
	}
	var c Cache
	if err := gob.NewDecoder(bytes.NewReader(b)).Decode(&c); err != nil {
		return nil, fmt.Errorf("decode cache: %w", err)
	}
	return &c, nil
}

func (cm *CacheManager) Set(ctx context.Context, c *Cache) error {
	conn, err := cm.rp.GetContext(ctx)
	if err != nil {
		return fmt.Errorf("get redis conn: %w", err)
	}
	defer conn.Close()
	buf := &bytes.Buffer{}
	if err := gob.NewEncoder(buf).Encode(c); err != nil {
		return fmt.Errorf("encode cache: %w", err)
	}
	if _, err := conn.Do("SET", cm.key, buf.Bytes()); err != nil {
		return err
	}
	return nil
}
