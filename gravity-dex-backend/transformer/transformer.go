package transformer

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"

	jsoniter "github.com/json-iterator/go"
	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/mongo"
	"go.uber.org/zap"
	"golang.org/x/sync/errgroup"

	"github.com/b-harvest/gravity-dex-backend/config"
	"github.com/b-harvest/gravity-dex-backend/schema"
	"github.com/b-harvest/gravity-dex-backend/service/store"
)

var jsonit = jsoniter.ConfigCompatibleWithStandardLibrary

type Transformer struct {
	cfg    config.TransformerConfig
	ss     *store.Service
	logger *zap.Logger
}

func New(cfg config.TransformerConfig, ss *store.Service, logger *zap.Logger) (*Transformer, error) {
	return &Transformer{cfg: cfg, ss: ss, logger: logger}, nil
}

func (t *Transformer) Run(ctx context.Context) error {
	for {
		t.logger.Debug("getting latest block height")
		h, err := t.ss.LatestBlockHeight(ctx)
		if err != nil {
			return fmt.Errorf("get latest block height: %w", err)
		}
		t.logger.Debug("got latest block height", zap.Int64("height", h))
		if h > 1 {
			t.logger.Debug("pruning outdated states", zap.Int64("height", h))
			if err := t.PruneOutdatedStates(ctx, h); err != nil {
				return fmt.Errorf("prune old state: %w", err)
			}
		}
		updates, err := t.AccStateUpdates(ctx, h+1)
		if err != nil {
			return fmt.Errorf("accumulate state updates: %w", err)
		}
		lastH := updates.lastBlockData.Header.Height
		t.logger.Info("updating state", zap.Int64("from", h+1), zap.Int64("to", lastH))
		if err := t.UpdateState(ctx, h, updates); err != nil {
			return fmt.Errorf("update state: %w", err)
		}
		t.logger.Debug("updating latest block height", zap.Int64("height", lastH))
		if err := t.ss.SetLatestBlockHeight(ctx, lastH); err != nil {
			return fmt.Errorf("update latest block height: %w", err)
		}
	}
}

func (t *Transformer) PruneOutdatedStates(ctx context.Context, currentBlockHeight int64) error {
	if err := t.ss.DeleteOutdatedAccountStatuses(ctx, currentBlockHeight); err != nil {
		return fmt.Errorf("delete outdated accounts: %w", err)
	}
	if err := t.ss.DeleteOutdatedPoolStatuses(ctx, currentBlockHeight); err != nil {
		return fmt.Errorf("delete outdated pools: %w", err)
	}
	return nil
}

func (t *Transformer) blockDataFilename(blockHeight int64) string {
	bs := int64(t.cfg.BlockDataBucketSize)
	p := blockHeight / bs * bs
	return filepath.Join(t.cfg.BlockDataDir, fmt.Sprintf(t.cfg.BlockDataFilename, p, blockHeight))
}

type BlockDataDecodeError struct {
	Err error
}

func (err *BlockDataDecodeError) Error() string {
	return err.Err.Error()
}

func (err *BlockDataDecodeError) Unwrap() error {
	return err.Err
}

func (t *Transformer) ReadBlockData(blockHeight int64) (*BlockData, error) {
	f, err := os.Open(t.blockDataFilename(blockHeight))
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var data BlockData
	if err := jsonit.NewDecoder(f).Decode(&data); err != nil {
		return nil, &BlockDataDecodeError{err}
	}
	return &data, nil
}

func (t *Transformer) WaitForBlockData(ctx context.Context, blockHeight int64, timeout time.Duration) (*BlockData, error) {
	if timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
	}
	for {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		default:
		}
		data, err := t.ReadBlockData(blockHeight)
		if err != nil {
			var berr *BlockDataDecodeError
			if !os.IsNotExist(err) && !errors.As(err, &berr) {
				//if !os.IsNotExist(err) {
				return nil, fmt.Errorf("read block data: %w", err)
			}
		} else {
			return data, nil
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(t.cfg.BlockDataWaitingInterval):
		}
	}
}

func (t *Transformer) UpdateState(ctx context.Context, currentBlockHeight int64, updates *StateUpdates) error {
	eg, ctx2 := errgroup.WithContext(ctx)
	eg.Go(func() error {
		if err := t.UpdateAccountStatus(ctx2, currentBlockHeight, updates); err != nil {
			return fmt.Errorf("update accounts: %w", err)
		}
		return nil
	})
	eg.Go(func() error {
		if err := t.UpdatePoolStatus(ctx2, currentBlockHeight, updates); err != nil {
			return fmt.Errorf("update pools: %w", err)
		}
		return nil
	})
	if updates.lastBankModuleState != nil {
		eg.Go(func() error {
			if err := t.UpdateBalancesAndSupplies(ctx2, updates); err != nil {
				return fmt.Errorf("update balances and supplies: %w", err)
			}
			return nil
		})
	}
	return eg.Wait()
}

func (t *Transformer) UpdateAccountStatus(ctx context.Context, currentBlockHeight int64, updates *StateUpdates) error {
	data := updates.lastBlockData
	lastBlockHeight := data.Header.Height
	reserveAccAddrs := make(map[string]struct{})
	for _, p := range data.Pools {
		reserveAccAddrs[p.ReserveAccountAddress] = struct{}{}
	}
	addrsToUpdate := make(map[string]struct{})
	for addr := range updates.depositStatusByAddress {
		addrsToUpdate[addr] = struct{}{}
	}
	for addr := range updates.swapStatusByAddress {
		addrsToUpdate[addr] = struct{}{}
	}
	var writes []mongo.WriteModel
	for addr := range addrsToUpdate {
		if _, ok := reserveAccAddrs[addr]; ok {
			continue
		}
		var accStatus schema.AccountStatus
		if currentBlockHeight > 0 {
			var err error
			accStatus, err = t.ss.AccountStatus(ctx, currentBlockHeight, addr)
			if err != nil && !errors.Is(err, mongo.ErrNoDocuments) {
				return fmt.Errorf("find account status: %w", err)
			}
		}
		accStatus.Deposits = schema.MergeAccountActionStatuses(accStatus.Deposits, updates.depositStatusByAddress[addr])
		accStatus.Swaps = schema.MergeAccountActionStatuses(accStatus.Swaps, updates.swapStatusByAddress[addr])
		writes = append(writes,
			mongo.NewUpdateOneModel().
				SetFilter(bson.M{
					schema.AccountStatusBlockHeightKey: lastBlockHeight,
					schema.AccountStatusAddressKey:     addr,
				}).
				SetUpdate(bson.M{"$set": bson.M{
					schema.AccountStatusDepositsKey: accStatus.Deposits,
					schema.AccountStatusSwapsKey:    accStatus.Swaps,
				}}).
				SetUpsert(true))
	}
	if len(writes) > 0 {
		if _, err := t.ss.AccountStatusCollection().BulkWrite(ctx, writes); err != nil {
			return fmt.Errorf("bulk write: %w", err)
		}
	}
	cur, err := t.ss.AccountStatusCollection().Find(ctx, bson.M{
		schema.AccountStatusBlockHeightKey: currentBlockHeight,
	})
	if err != nil {
		return fmt.Errorf("find account statuses: %w", err)
	}
	defer cur.Close(ctx)
	writes = nil
	for cur.Next(ctx) {
		var accStatus schema.AccountStatus
		if err := cur.Decode(&accStatus); err != nil {
			return fmt.Errorf("decode account status: %w", err)
		}
		if _, ok := addrsToUpdate[accStatus.Address]; ok {
			continue
		}
		writes = append(writes, mongo.NewUpdateOneModel().
			SetFilter(bson.M{
				schema.AccountStatusBlockHeightKey: lastBlockHeight,
				schema.AccountStatusAddressKey:     accStatus.Address,
			}).
			SetUpdate(bson.M{"$set": bson.M{
				schema.AccountStatusDepositsKey: accStatus.Deposits,
				schema.AccountStatusSwapsKey:    accStatus.Swaps,
			}}).
			SetUpsert(true))
	}
	if len(writes) > 0 {
		if _, err := t.ss.AccountStatusCollection().BulkWrite(ctx, writes); err != nil {
			return fmt.Errorf("bulk write: %w", err)
		}
	}
	return nil
}

func (t *Transformer) UpdatePoolStatus(ctx context.Context, currentBlockHeight int64, updates *StateUpdates) error {
	data := updates.lastBlockData
	lastBlockHeight := data.Header.Height
	var writes, writes2 []mongo.WriteModel
	for _, p := range data.Pools {
		var poolStatus schema.PoolStatus
		if currentBlockHeight > 0 {
			var err error
			poolStatus, err = t.ss.PoolStatus(ctx, currentBlockHeight, p.Id)
			if err != nil && !errors.Is(err, mongo.ErrNoDocuments) {
				return fmt.Errorf("find pool: %w", err)
			}
		}
		poolStatus.SwapFeeVolumes = schema.MergeVolumes(poolStatus.SwapFeeVolumes, updates.swapVolumesByPoolID[p.Id])
		poolStatus.SwapFeeVolumes.RemoveOutdated(data.Header.Time.Add(-time.Hour))
		writes = append(writes, mongo.NewUpdateOneModel().
			SetFilter(bson.M{
				schema.PoolStatusBlockHeightKey: lastBlockHeight,
				schema.PoolStatusIDKey:          p.Id,
			}).
			SetUpdate(bson.M{
				"$set": bson.M{
					schema.PoolStatusSwapFeeVolumesKey: poolStatus.SwapFeeVolumes,
				},
			}).
			SetUpsert(true))
		writes2 = append(writes2, mongo.NewUpdateOneModel().
			SetFilter(bson.M{
				schema.PoolIDKey: p.Id,
			}).
			SetUpdate(bson.M{
				"$set": bson.M{
					schema.PoolReserveAccountAddressKey: p.ReserveAccountAddress,
					schema.PoolReserveCoinDenomsKey:     p.ReserveCoinDenoms,
					schema.PoolPoolCoinDenomKey:         p.PoolCoinDenom,
				},
			}).
			SetUpsert(true))
	}
	if len(writes) > 0 {
		if _, err := t.ss.PoolStatusCollection().BulkWrite(ctx, writes); err != nil {
			return fmt.Errorf("bulk write: %w", err)
		}
	}
	if len(writes2) > 0 {
		if _, err := t.ss.PoolCollection().BulkWrite(ctx, writes2); err != nil {
			return fmt.Errorf("bulk write: %w", err)
		}
	}
	return nil
}

func (t *Transformer) UpdateBalancesAndSupplies(ctx context.Context, updates *StateUpdates) error {
	bankModuleState := updates.lastBankModuleState
	if bankModuleState == nil {
		return nil
	}
	lastBankModuleStateHeight := updates.lastBankModuleStateHeight
	var writes []mongo.WriteModel
	for _, b := range bankModuleState.Balances {
		writes = append(writes, mongo.NewUpdateOneModel().
			SetFilter(bson.M{
				schema.BalanceAddressKey: b.Address,
			}).
			SetUpdate(bson.M{
				"$set": bson.M{
					schema.BalanceBlockHeightKey: lastBankModuleStateHeight,
					schema.BalanceCoinsKey:       schema.CoinsFromSDK(b.Coins),
				},
			}).
			SetUpsert(true))
	}
	if len(writes) > 0 {
		if _, err := t.ss.BalanceCollection().BulkWrite(ctx, writes); err != nil {
			return fmt.Errorf("bulk write: %w", err)
		}
	}
	writes = nil
	for _, c := range bankModuleState.Supply {
		writes = append(writes, mongo.NewUpdateOneModel().
			SetFilter(bson.M{
				schema.SupplyDenomKey: c.Denom,
			}).
			SetUpdate(bson.M{
				"$set": bson.M{
					schema.SupplyBlockHeightKey: lastBankModuleStateHeight,
					schema.SupplyAmountKey:      c.Amount.Int64(),
				},
			}).
			SetUpsert(true))
	}
	if len(writes) > 0 {
		if _, err := t.ss.SupplyCollection().BulkWrite(ctx, writes); err != nil {
			return fmt.Errorf("bulk write: %w", err)
		}
	}
	return nil
}
