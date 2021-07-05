package store

import (
	"context"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"

	"github.com/b-harvest/gravity-dex-backend/config"
	"github.com/b-harvest/gravity-dex-backend/schema"
)

func TestService_Banner(t *testing.T) {
	cfg := config.DefaultMongoDBConfig
	cfg.DB = "test"

	mc, err := mongo.Connect(context.Background(), options.Client().ApplyURI(cfg.URI))
	require.NoError(t, err)
	defer mc.Disconnect(context.Background())

	s := NewService(config.DefaultMongoDBConfig, mc)

	err = s.BannerCollection().Drop(context.Background())
	require.NoError(t, err)

	_, err = s.BannerCollection().InsertMany(context.Background(), bson.A{
		schema.Banner{
			Text:      "passive banner 1",
			VisibleAt: time.Date(2021, time.May, 4, 0, 0, 0, 0, time.UTC),
			StartsAt:  time.Date(2021, time.May, 4, 0, 0, 0, 0, time.UTC),
			EndsAt:    time.Date(2021, time.May, 4, 12, 0, 0, 0, time.UTC),
		},
		schema.Banner{
			Text:      "event 1",
			VisibleAt: time.Date(2021, time.May, 4, 8, 30, 0, 0, time.UTC),
			StartsAt:  time.Date(2021, time.May, 4, 9, 0, 0, 0, time.UTC),
			EndsAt:    time.Date(2021, time.May, 4, 9, 10, 0, 0, time.UTC),
		},
		schema.Banner{
			Text:      "event 2",
			VisibleAt: time.Date(2021, time.May, 4, 17, 30, 0, 0, time.UTC),
			StartsAt:  time.Date(2021, time.May, 4, 18, 0, 0, 0, time.UTC),
			EndsAt:    time.Date(2021, time.May, 4, 18, 10, 0, 0, time.UTC),
		},
	})
	require.NoError(t, err)

	for _, tc := range []struct {
		now  time.Time
		text string
	}{
		{time.Date(2021, time.May, 4, 0, 0, 0, 0, time.UTC), "passive banner 1"},
		{time.Date(2021, time.May, 4, 8, 29, 59, 0, time.UTC), "passive banner 1"},
		{time.Date(2021, time.May, 4, 8, 30, 0, 0, time.UTC), "event 1"},
		{time.Date(2021, time.May, 4, 9, 9, 59, 0, time.UTC), "event 1"},
		{time.Date(2021, time.May, 4, 9, 10, 0, 0, time.UTC), "passive banner 1"},
		{time.Date(2021, time.May, 4, 11, 59, 59, 0, time.UTC), "passive banner 1"},
		{time.Date(2021, time.May, 4, 12, 0, 0, 0, time.UTC), ""},
		{time.Date(2021, time.May, 4, 17, 29, 59, 0, time.UTC), ""},
		{time.Date(2021, time.May, 4, 17, 30, 0, 0, time.UTC), "event 2"},
		{time.Date(2021, time.May, 4, 18, 9, 59, 0, time.UTC), "event 2"},
		{time.Date(2021, time.May, 4, 18, 10, 0, 0, time.UTC), ""},
	} {
		b, err := s.Banner(context.Background(), tc.now)
		require.NoError(t, err)
		if tc.text == "" {
			require.Nil(t, b)
		} else {
			require.Equal(t, tc.text, b.Text)
		}
	}
}
