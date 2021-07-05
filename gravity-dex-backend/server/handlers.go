package server

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/gomodule/redigo/redis"
	"github.com/labstack/echo/v4"
	"go.mongodb.org/mongo-driver/mongo"

	"github.com/b-harvest/gravity-dex-backend/schema"
)

func (s *Server) registerRoutes() {
	s.GET("/status", s.GetStatus)
	s.GET("/scoreboard", s.GetScoreBoard)
	s.GET("/scoreboard/search", s.SearchAccount)
	s.GET("/actions", s.GetActionStatus)
	s.GET("/pools", s.GetPools)
	s.GET("/prices", s.GetPrices)
	s.GET("/banner", s.GetBanner)
}

func (s *Server) GetStatus(c echo.Context) error {
	blockHeight, err := s.ss.LatestBlockHeight(c.Request().Context())
	if err != nil {
		return fmt.Errorf("get latest block height: %w", err)
	}
	return c.JSON(http.StatusOK, schema.GetStatusResponse{
		LatestBlockHeight: blockHeight,
	})
}

func (s *Server) GetScoreBoard(c echo.Context) error {
	var req schema.GetScoreBoardRequest
	if err := c.Bind(&req); err != nil {
		return err
	}
	var sbCache schema.ScoreBoardCache
	if err := RetryLoadingCache(c.Request().Context(), func(ctx context.Context) error {
		var err error
		sbCache, err = s.LoadScoreBoardCache(ctx)
		return err
	}, s.cfg.CacheLoadTimeout); err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			return echo.NewHTTPError(http.StatusInternalServerError, "no score board data found")
		}
		return fmt.Errorf("load score board cache: %w", err)
	}
	resp := schema.GetScoreBoardResponse{
		BlockHeight: sbCache.BlockHeight,
		Accounts:    []schema.GetScoreBoardResponseAccount{},
		UpdatedAt:   sbCache.UpdatedAt,
	}
	for _, acc := range sbCache.Accounts {
		resp.Accounts = append(resp.Accounts, schema.GetScoreBoardResponseAccount{
			Ranking:      acc.Ranking,
			Username:     acc.Username,
			Address:      acc.Address,
			TotalScore:   acc.TotalScore,
			TradingScore: acc.TradingScore,
			ActionScore:  acc.ActionScore,
			IsValid:      acc.IsValid,
		})
	}
	if req.Address != "" {
		accCache, err := s.LoadAccountCache(c.Request().Context(), req.Address)
		if err != nil {
			if !errors.Is(err, redis.ErrNil) {
				return fmt.Errorf("load account cache: %w", err)
			}
		} else {
			resp.Me = &schema.GetScoreBoardResponseAccount{
				Ranking:      accCache.Ranking,
				Username:     accCache.Username,
				Address:      accCache.Address,
				TotalScore:   accCache.TotalScore,
				TradingScore: accCache.TradingScore,
				ActionScore:  accCache.ActionScore,
				IsValid:      accCache.IsValid,
			}
		}
	}
	return c.JSON(http.StatusOK, resp)
}

func (s *Server) SearchAccount(c echo.Context) error {
	var req schema.SearchAccountRequest
	if err := c.Bind(&req); err != nil {
		return err
	}
	if req.Query == "" {
		return echo.NewHTTPError(http.StatusBadRequest, "query must be provided")
	}
	var address string
	if strings.HasPrefix(req.Query, s.cfg.AddressPrefix) {
		address = req.Query
	} else {
		acc, err := s.ss.AccountByUsername(c.Request().Context(), req.Query)
		if err != nil {
			if !errors.Is(err, mongo.ErrNoDocuments) {
				return fmt.Errorf("get account by username: %w", err)
			}
		} else {
			address = acc.Address
		}
	}
	if address == "" {
		return c.JSON(http.StatusOK, schema.SearchAccountResponse{})
	}
	accCache, err := s.LoadAccountCache(c.Request().Context(), address)
	if err != nil {
		if errors.Is(err, redis.ErrNil) {
			return c.JSON(http.StatusOK, schema.SearchAccountResponse{})
		}
		return fmt.Errorf("load account cache: %w", err)
	}
	return c.JSON(http.StatusOK, schema.SearchAccountResponse{
		BlockHeight: accCache.BlockHeight,
		Account: &schema.GetScoreBoardResponseAccount{
			Ranking:      accCache.Ranking,
			Username:     accCache.Username,
			Address:      accCache.Address,
			TotalScore:   accCache.TotalScore,
			TradingScore: accCache.TradingScore,
			ActionScore:  accCache.ActionScore,
			IsValid:      accCache.IsValid,
		},
		UpdatedAt: accCache.UpdatedAt,
	})
}

func (s *Server) GetActionStatus(c echo.Context) error {
	var req schema.GetActionStatusRequest
	if err := c.Bind(&req); err != nil {
		return err
	}
	if req.Address == "" {
		return echo.NewHTTPError(http.StatusBadRequest, "address must be provided")
	}
	accCache, err := s.LoadAccountCache(c.Request().Context(), req.Address)
	if err != nil {
		if errors.Is(err, redis.ErrNil) {
			return c.JSON(http.StatusOK, schema.GetActionStatusResponse{})
		}
		return fmt.Errorf("load account cache: %w", err)
	}
	todayKey := time.Now().UTC().Format("2006-01-02")
	return c.JSON(http.StatusOK, schema.GetActionStatusResponse{
		BlockHeight: accCache.BlockHeight,
		Account: &schema.GetActionStatusResponseAccount{
			Deposit: schema.GetActionStatusResponseStatus{
				NumDifferentPools:         accCache.DepositStatus.NumDifferentPools,
				NumDifferentPoolsToday:    accCache.DepositStatus.NumDifferentPoolsByDate[todayKey],
				MaxNumDifferentPoolsToday: s.cfg.Score.MaxActionScorePerDay,
			},
			Swap: schema.GetActionStatusResponseStatus{
				NumDifferentPools:         accCache.SwapStatus.NumDifferentPools,
				NumDifferentPoolsToday:    accCache.SwapStatus.NumDifferentPoolsByDate[todayKey],
				MaxNumDifferentPoolsToday: s.cfg.Score.MaxActionScorePerDay,
			},
		},
		UpdatedAt: accCache.UpdatedAt,
	})
}

func (s *Server) GetPools(c echo.Context) error {
	var cache schema.PoolsCache
	if err := RetryLoadingCache(c.Request().Context(), func(ctx context.Context) error {
		var err error
		cache, err = s.LoadPoolsCache(ctx)
		return err
	}, s.cfg.CacheLoadTimeout); err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			return echo.NewHTTPError(http.StatusInternalServerError, "no pool data found")
		}
		return fmt.Errorf("load pools cache: %w", err)
	}
	return c.JSON(http.StatusOK, schema.GetPoolsResponse(cache))
}

func (s *Server) GetPrices(c echo.Context) error {
	var cache schema.PricesCache
	if err := RetryLoadingCache(c.Request().Context(), func(ctx context.Context) error {
		var err error
		cache, err = s.LoadPricesCache(ctx)
		return err
	}, s.cfg.CacheLoadTimeout); err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			return echo.NewHTTPError(http.StatusInternalServerError, "no price data found")
		}
		return fmt.Errorf("load prices cache: %w", err)
	}
	return c.JSON(http.StatusOK, schema.GetPricesResponse(cache))
}

func (s *Server) GetBanner(c echo.Context) error {
	banner, err := s.ss.Banner(c.Request().Context(), time.Now())
	if err != nil {
		return fmt.Errorf("get banner: %w", err)
	}
	resp := schema.GetBannerResponse{}
	if banner != nil {
		var state schema.GetBannerResponseState
		if banner.StartsAt.After(time.Now()) {
			state = schema.GetBannerResponseStateUpcoming
		} else {
			state = schema.GetBannerResponseStateStarted
		}
		var text string
		switch state {
		case schema.GetBannerResponseStateUpcoming:
			text = banner.UpcomingText
		case schema.GetBannerResponseStateStarted:
			text = banner.Text
		}
		resp.Banner = &schema.GetBannerResponseBanner{
			State:    state,
			Text:     text,
			URL:      banner.URL,
			StartsAt: banner.StartsAt,
			EndsAt:   banner.EndsAt,
		}
	}
	return c.JSON(http.StatusOK, resp)
}
