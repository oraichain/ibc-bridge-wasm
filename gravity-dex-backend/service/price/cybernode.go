package price

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

var _ Service = (*CyberNodeService)(nil)

type CyberNodeService struct {
	hc *http.Client
	cs *CacheStorage
}

func NewCyberNodeService(updateInterval time.Duration) *CyberNodeService {
	return &CyberNodeService{&http.Client{}, NewCacheStorage(updateInterval)}
}

func (s *CyberNodeService) Symbols() []string {
	return []string{"gcyb"}
}

func (s *CyberNodeService) Prices(ctx context.Context, symbols ...string) (Table, error) {
	if len(symbols) > 1 || (len(symbols) == 1 && strings.ToLower(symbols[0]) != "gcyb") {
		return nil, fmt.Errorf("only \"gcyb\" symbol can be queried through CyberNodeService")
	}
	if len(s.cs.NewSymbols("gcyb")) > 0 {
		req, err := http.NewRequestWithContext(ctx, "GET", "https://market-data.cybernode.ai/api/coins/cyb", nil)
		if err != nil {
			return nil, fmt.Errorf("new request: %w", err)
		}
		resp, err := s.hc.Do(req)
		if err != nil {
			return nil, err
		}
		defer resp.Body.Close()
		defer io.Copy(io.Discard, resp.Body)
		var r CyberNodeResponse
		if err := json.NewDecoder(resp.Body).Decode(&r); err != nil {
			return nil, fmt.Errorf("unmarshal data: %w", err)
		}
		s.cs.SetPrice("gcyb", r.MarketData.CurrentPrice.USD)
	}
	p, ok := s.cs.Price("gcyb")
	if !ok { // this will never happen
		return nil, fmt.Errorf("cache for symbol \"gcyb\" not found")
	}
	return Table{"gcyb": p}, nil
}

type CyberNodeResponse struct {
	MarketData struct {
		CurrentPrice struct {
			USD float64 `json:"usd"`
		} `json:"current_price"`
	} `json:"market_data"`
}
