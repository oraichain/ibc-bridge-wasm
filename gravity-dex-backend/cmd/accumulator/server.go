package main

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
)

type Server struct {
	*echo.Echo
	cm *CacheManager
}

func NewServer(cm *CacheManager) *Server {
	e := echo.New()
	e.Use(middleware.Recover())
	e.Use(middleware.CORS())
	e.HideBanner = true
	e.HidePort = true

	s := &Server{
		Echo: e,
		cm:   cm,
	}
	s.registerRoutes()
	return s
}

func (s *Server) registerRoutes() {
	s.GET("/stats", s.GetStats)
}

func (s *Server) GetStats(c echo.Context) error {
	cache, err := s.cm.Get(c.Request().Context())
	if err != nil {
		return fmt.Errorf("get cache: %w", err)
	}
	if cache == nil {
		return echo.NewHTTPError(http.StatusServiceUnavailable, "stats not found")
	}
	var resp struct {
		BlockHeight                   int64  `json:"blockHeight"`
		NumActiveAddresses            int    `json:"numActiveAddresses"`
		NumActiveAddressesLast24Hours int    `json:"numActiveAddressesLast24Hours"`
		NumDeposits                   int    `json:"numDeposits"`
		NumSwaps                      int    `json:"numSwaps"`
		NumTransactions               int    `json:"numTransactions"`
		NumDepositsLast24Hours        int    `json:"numDepositsLast24Hours"`
		NumSwapsLast24Hours           int    `json:"numSwapsLast24Hours"`
		NumTransactionsLast24Hours    int    `json:"numTransactionsLast24Hours"`
		TransactedCoins               string `json:"transactedCoins"`
		TransactedCoinsLast24Hours    string `json:"transactedCoinsLast24Hours"`
		SwapVolume                    string `json:"swapVolume"`
		SwapVolumeLast24Hours         string `json:"swapVolumeLast24Hours"`
	}
	return c.JSON(http.StatusOK, resp)
}

func (s *Server) ShutdownWithTimeout(timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	return s.Shutdown(ctx)
}
