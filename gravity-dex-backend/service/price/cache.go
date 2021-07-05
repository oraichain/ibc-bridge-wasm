package price

import (
	"strings"
	"time"
)

type CacheStorage struct {
	c      map[string]Cache
	maxAge time.Duration
}

func NewCacheStorage(maxAge time.Duration) *CacheStorage {
	return &CacheStorage{make(map[string]Cache), maxAge}
}

func (cs *CacheStorage) Expire() {
	now := time.Now()
	for k, c := range cs.c {
		if !c.UpdatedAt.Add(cs.maxAge).After(now) {
			delete(cs.c, k)
		}
	}
}

func (cs *CacheStorage) NewSymbols(symbols ...string) []string {
	cs.Expire()
	var res []string
	for _, s := range symbols {
		if _, ok := cs.c[s]; !ok {
			res = append(res, s)
		}
	}
	return res
}

func (cs *CacheStorage) SetPrice(symbol string, price float64) {
	cs.c[strings.ToLower(symbol)] = Cache{price, time.Now()}
}

func (cs *CacheStorage) Price(symbol string) (float64, bool) {
	c, ok := cs.c[symbol]
	if !ok {
		return 0, false
	}
	return c.Price, true
}

type Cache struct {
	Price     float64
	UpdatedAt time.Time
}
