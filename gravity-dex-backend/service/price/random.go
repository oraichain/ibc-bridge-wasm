package price

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
)

var _ Service = (*RandomOracleService)(nil)

type RandomOracleService struct {
	apiURL *url.URL
	hc     *http.Client
}

func NewRandomOracleService(apiURL string) (*RandomOracleService, error) {
	u, err := url.Parse(apiURL)
	if err != nil {
		return nil, fmt.Errorf("parse api url: %w", err)
	}
	return &RandomOracleService{apiURL: u, hc: &http.Client{}}, nil
}

func (s *RandomOracleService) Symbols() []string {
	return []string{"earth", "uusd"}
}

func (s *RandomOracleService) Prices(ctx context.Context, symbols ...string) (Table, error) {
	u := *s.apiURL
	u.RawQuery = url.Values{"symbols": {strings.Join(symbols, ",")}}.Encode()
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
	var r RandomOracleResponse
	if err := json.NewDecoder(resp.Body).Decode(&r); err != nil {
		return nil, fmt.Errorf("unmarshal data: %w", err)
	}
	t := make(Table)
	for symbol, coin := range r.Coins {
		t[strings.ToLower(symbol)] = coin.Price
	}
	return t, nil
}

type RandomOracleResponse struct {
	Coins map[string]struct {
		Price float64 `json:"price"`
	} `json:"coins"`
}
