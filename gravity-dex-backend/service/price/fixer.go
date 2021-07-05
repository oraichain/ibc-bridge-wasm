package price

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

const (
	fixerAPIBaseURL = "https://data.fixer.io/api/latest"
)

var _ Service = (*FixerService)(nil)

type FixerService struct {
	apiURL    *url.URL
	hc        *http.Client
	accessKey string
	cs        *CacheStorage
}

func NewFixerService(accessKey string, updateInterval time.Duration) (*FixerService, error) {
	u, err := url.Parse(fixerAPIBaseURL)
	if err != nil {
		return nil, fmt.Errorf("parse api base url: %w", err)
	}
	return &FixerService{
		apiURL:    u,
		hc:        &http.Client{},
		accessKey: accessKey,
		cs:        NewCacheStorage(updateInterval),
	}, nil
}

func (s *FixerService) Symbols() []string {
	return []string{"com"}
}

func (s *FixerService) Prices(ctx context.Context, symbols ...string) (Table, error) {
	if len(symbols) > 1 || (len(symbols) == 1 && strings.ToLower(symbols[0]) != "com") {
		return nil, fmt.Errorf("only \"com\" symbol can be queried through FixerService")
	}
	if len(s.cs.NewSymbols("com")) > 0 {
		u := *s.apiURL
		u.RawQuery = url.Values{"access_key": {s.accessKey}, "base": {"eur"}, "symbols": {"usd"}}.Encode()
		req, err := http.NewRequestWithContext(ctx, "GET", u.String(), nil)
		if err != nil {
			return nil, fmt.Errorf("new request: %w", err)
		}
		resp, err := s.hc.Do(req)
		if err != nil {
			return nil, err
		}
		defer resp.Body.Close()
		defer io.Copy(io.Discard, resp.Body)
		var r FixerResponse
		if err := json.NewDecoder(resp.Body).Decode(&r); err != nil {
			return nil, fmt.Errorf("unmarshal data: %w", err)
		}
		s.cs.SetPrice("com", r.Rates.USD)
	}
	p, ok := s.cs.Price("com")
	if !ok { // this will never happen
		return nil, fmt.Errorf("cache for symbol \"com\" not found")
	}
	return Table{"com": p}, nil
}

type FixerResponse struct {
	Success bool `json:"success"`
	Rates   struct {
		USD float64 `json:"USD"`
	} `json:"rates"`
}
