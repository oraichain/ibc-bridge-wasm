package server

import (
	"context"
	"time"

	"github.com/gomodule/redigo/redis"
	"github.com/labstack/echo/v4"
	"github.com/labstack/echo/v4/middleware"
	"go.uber.org/zap"

	"github.com/b-harvest/gravity-dex-backend/config"
	"github.com/b-harvest/gravity-dex-backend/service/price"
	"github.com/b-harvest/gravity-dex-backend/service/pricetable"
	"github.com/b-harvest/gravity-dex-backend/service/score"
	"github.com/b-harvest/gravity-dex-backend/service/store"
)

type Server struct {
	*echo.Echo
	cfg    config.ServerConfig
	ss     *store.Service
	ps     price.Service
	pts    *pricetable.Service
	scs    *score.Service
	rp     *redis.Pool
	logger *zap.Logger
}

func New(cfg config.ServerConfig, ss *store.Service, ps price.Service, pts *pricetable.Service, scs *score.Service, rp *redis.Pool, logger *zap.Logger) *Server {
	e := echo.New()
	e.HideBanner = true
	e.HidePort = true
	e.Debug = cfg.Debug
	e.Use(middleware.Logger())
	e.Use(middleware.Recover())
	e.Use(middleware.CORS())
	s := &Server{e, cfg, ss, ps, pts, scs, rp, logger}
	s.registerRoutes()
	return s
}

func (s *Server) ShutdownWithTimeout(timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	return s.Shutdown(ctx)
}
