package store

import (
	"context"
	"errors"
	"fmt"
	"time"

	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"

	"github.com/b-harvest/gravity-dex-backend/schema"
)

type Service struct {
	cfg Config
	mc  *mongo.Client
}

func NewService(cfg Config, mc *mongo.Client) *Service {
	return &Service{cfg, mc}
}

func (s *Service) Database() *mongo.Database {
	return s.mc.Database(s.cfg.DB)
}

func (s *Service) CheckpointCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.CheckpointCollection)
}

func (s *Service) AccountCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.AccountCollection)
}

func (s *Service) AccountStatusCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.AccountStatusCollection)
}

func (s *Service) PoolCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.PoolCollection)
}

func (s *Service) PoolStatusCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.PoolStatusCollection)
}

func (s *Service) BalanceCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.BalanceCollection)
}

func (s *Service) SupplyCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.SupplyCollection)
}

func (s *Service) BannerCollection() *mongo.Collection {
	return s.Database().Collection(s.cfg.BannerCollection)
}

func (s *Service) EnsureDBIndexes(ctx context.Context) ([]string, error) {
	var res []string
	for _, x := range []struct {
		coll *mongo.Collection
		is   []mongo.IndexModel
	}{
		{s.AccountCollection(), []mongo.IndexModel{
			{Keys: bson.D{{schema.AccountAddressKey, 1}}},
			{Keys: bson.D{{schema.AccountUsernameKey, 1}}},
		}},
		{s.AccountStatusCollection(), []mongo.IndexModel{
			{Keys: bson.D{{schema.AccountStatusAddressKey, 1}}},
			{Keys: bson.D{{schema.AccountStatusBlockHeightKey, 1}}},
			{Keys: bson.D{{schema.AccountStatusBlockHeightKey, 1}, {schema.AccountStatusAddressKey, 1}}},
		}},
		{s.PoolStatusCollection(), []mongo.IndexModel{
			{Keys: bson.D{{schema.PoolStatusIDKey, 1}}},
			{Keys: bson.D{{schema.PoolStatusBlockHeightKey, 1}}},
			{Keys: bson.D{{schema.PoolStatusBlockHeightKey, 1}, {schema.PoolStatusIDKey, 1}}},
		}},
		{s.BalanceCollection(), []mongo.IndexModel{
			{Keys: bson.D{{schema.BalanceAddressKey, 1}}},
		}},
		{s.SupplyCollection(), []mongo.IndexModel{
			{Keys: bson.D{{schema.SupplyDenomKey, 1}}},
		}},
		{s.BannerCollection(), []mongo.IndexModel{
			{Keys: bson.D{{schema.BannerVisibleAtKey, 1}}},
			{Keys: bson.D{{schema.BannerStartsAtKey, 1}}},
			{Keys: bson.D{{schema.BannerEndsAtKey, 1}}},
		}},
	} {
		names, err := x.coll.Indexes().CreateMany(ctx, x.is)
		if err != nil {
			return res, err
		}
		res = append(res, names...)
	}
	return res, nil
}

func (s *Service) LatestBlockHeight(ctx context.Context) (int64, error) {
	var cp schema.Checkpoint
	if err := s.CheckpointCollection().FindOne(ctx, bson.M{
		schema.CheckpointBlockHeightKey: bson.M{"$exists": true},
	}).Decode(&cp); err != nil {
		if errors.Is(err, mongo.ErrNoDocuments) {
			return 0, nil
		}
		return 0, err
	}
	return cp.BlockHeight, nil
}

func (s *Service) SetLatestBlockHeight(ctx context.Context, height int64) error {
	if _, err := s.CheckpointCollection().UpdateOne(ctx, bson.M{
		schema.CheckpointBlockHeightKey: bson.M{"$exists": true},
	}, bson.M{
		"$set": bson.M{
			schema.CheckpointBlockHeightKey: height,
			schema.CheckpointTimestampKey:   time.Now(),
		},
	}, options.Update().SetUpsert(true)); err != nil {
		return err
	}
	return nil
}

func (s *Service) DeleteOutdatedAccountStatuses(ctx context.Context, currentBlockHeight int64) error {
	if _, err := s.AccountStatusCollection().DeleteMany(ctx, bson.M{
		"$or": bson.A{
			bson.M{
				schema.AccountStatusBlockHeightKey: bson.M{"$lt": currentBlockHeight - 1},
			},
			bson.M{
				schema.AccountStatusBlockHeightKey: bson.M{"$gt": currentBlockHeight + 1},
			},
		},
	}); err != nil {
		return err
	}
	return nil
}

func (s *Service) DeleteOutdatedPoolStatuses(ctx context.Context, currentBlockHeight int64) error {
	if _, err := s.PoolStatusCollection().DeleteMany(ctx, bson.M{
		"$or": bson.A{
			bson.M{
				schema.PoolStatusBlockHeightKey: bson.M{"$lt": currentBlockHeight - 1},
			},
			bson.M{
				schema.PoolStatusBlockHeightKey: bson.M{"$gt": currentBlockHeight + 1},
			},
		},
	}); err != nil {
		return err
	}
	return nil
}

func (s *Service) AccountStatus(ctx context.Context, blockHeight int64, address string) (schema.AccountStatus, error) {
	var accStatus schema.AccountStatus
	if err := s.AccountStatusCollection().FindOne(ctx, bson.M{
		schema.AccountStatusBlockHeightKey: blockHeight,
		schema.AccountStatusAddressKey:     address,
	}).Decode(&accStatus); err != nil {
		return schema.AccountStatus{}, err
	}
	return accStatus, nil
}

func (s *Service) PoolStatus(ctx context.Context, blockHeight int64, id uint64) (schema.PoolStatus, error) {
	var poolStatus schema.PoolStatus
	if err := s.PoolStatusCollection().FindOne(ctx, bson.M{
		schema.PoolStatusBlockHeightKey: blockHeight,
		schema.PoolStatusIDKey:          id,
	}).Decode(&poolStatus); err != nil {
		return schema.PoolStatus{}, err
	}
	return poolStatus, nil
}

func (s *Service) AccountByUsername(ctx context.Context, username string) (schema.Account, error) {
	var acc schema.Account
	if err := s.AccountCollection().FindOne(ctx, bson.M{
		schema.AccountUsernameKey: username,
	}).Decode(&acc); err != nil {
		return schema.Account{}, err
	}
	return acc, nil
}

func (s *Service) IterateAccounts(ctx context.Context, blockHeight int64, cb func(schema.Account) (stop bool, err error)) error {
	cur, err := s.AccountCollection().Aggregate(ctx, bson.A{
		bson.M{
			"$match": bson.M{
				schema.AccountIsBlockedKey: bson.M{
					"$in": bson.A{false, nil},
				},
			},
		},
		bson.M{
			"$lookup": bson.M{
				"from":         s.cfg.BalanceCollection,
				"localField":   schema.AccountAddressKey,
				"foreignField": schema.BalanceAddressKey,
				"as":           schema.AccountBalanceKey,
			},
		},
		bson.M{
			"$unwind": "$" + schema.AccountBalanceKey,
		},
		bson.M{
			"$lookup": bson.M{
				"from":         s.cfg.AccountStatusCollection,
				"localField":   schema.AccountAddressKey,
				"foreignField": schema.AccountStatusAddressKey,
				"as":           schema.AccountStatusKey,
			},
		},
		bson.M{
			"$unwind": bson.M{
				"path":                       "$" + schema.AccountStatusKey,
				"preserveNullAndEmptyArrays": true,
			},
		},
		bson.M{
			"$match": bson.M{
				schema.AccountStatusKey + "." + schema.AccountStatusBlockHeightKey: bson.M{
					"$in": bson.A{blockHeight, nil},
				},
			},
		},
	})
	if err != nil {
		return fmt.Errorf("aggregate accounts: %w", err)
	}
	defer cur.Close(ctx)
	for cur.Next(ctx) {
		var acc schema.Account
		if err := cur.Decode(&acc); err != nil {
			return fmt.Errorf("decode account: %w", err)
		}
		stop, err := cb(acc)
		if err != nil {
			return err
		}
		if stop {
			break
		}
	}
	return nil
}

func (s *Service) Pools(ctx context.Context, blockHeight int64) ([]schema.Pool, error) {
	cur, err := s.PoolCollection().Aggregate(ctx, bson.A{
		bson.M{
			"$lookup": bson.M{
				"from":         s.cfg.BalanceCollection,
				"localField":   schema.PoolReserveAccountAddressKey,
				"foreignField": schema.BalanceAddressKey,
				"as":           schema.PoolReserveAccountBalanceKey,
			},
		},
		bson.M{
			"$unwind": "$" + schema.PoolReserveAccountBalanceKey,
		},
		bson.M{
			"$lookup": bson.M{
				"from":         s.cfg.SupplyCollection,
				"localField":   schema.PoolPoolCoinDenomKey,
				"foreignField": schema.SupplyDenomKey,
				"as":           schema.PoolPoolCoinSupplyKey,
			},
		},
		bson.M{
			"$unwind": "$" + schema.PoolPoolCoinSupplyKey,
		},
		bson.M{
			"$lookup": bson.M{
				"from":         s.cfg.PoolStatusCollection,
				"localField":   schema.PoolIDKey,
				"foreignField": schema.PoolStatusIDKey,
				"as":           schema.PoolStatusKey,
			},
		},
		bson.M{
			"$unwind": bson.M{
				"path":                       "$" + schema.PoolStatusKey,
				"preserveNullAndEmptyArrays": true,
			},
		},
		bson.M{
			"$match": bson.M{
				schema.PoolStatusKey + "." + schema.PoolStatusBlockHeightKey: bson.M{
					"$in": bson.A{blockHeight, nil},
				},
			},
		},
	})
	if err != nil {
		return nil, fmt.Errorf("aggregate pools: %w", err)
	}
	defer cur.Close(ctx)
	var ps []schema.Pool
	if err := cur.All(ctx, &ps); err != nil {
		return nil, fmt.Errorf("decode pools: %w", err)
	}
	return ps, nil
}

func (s *Service) Banner(ctx context.Context, now time.Time) (*schema.Banner, error) {
	var b schema.Banner
	if err := s.BannerCollection().FindOne(ctx, bson.M{
		schema.BannerVisibleAtKey: bson.M{
			"$lte": now,
		},
		schema.BannerEndsAtKey: bson.M{
			"$gt": now,
		},
	}, options.FindOne().SetSort(bson.M{schema.BannerStartsAtKey: -1})).Decode(&b); err != nil {
		if !errors.Is(err, mongo.ErrNoDocuments) {
			return nil, err
		}
		return nil, nil
	}
	return &b, nil
}
