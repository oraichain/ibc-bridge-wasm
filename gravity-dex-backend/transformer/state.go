package transformer

import (
	"context"
	"errors"
	"fmt"
	"time"

	sdk "github.com/cosmos/cosmos-sdk/types"
	banktypes "github.com/cosmos/cosmos-sdk/x/bank/types"
	liquiditytypes "github.com/tendermint/liquidity/x/liquidity/types"
	"go.uber.org/zap"

	"github.com/b-harvest/gravity-dex-backend/schema"
)

type StateUpdates struct {
	lastBlockData             *BlockData
	lastBankModuleStateHeight int64
	lastBankModuleState       *banktypes.GenesisState
	depositStatusByAddress    ActionStatusByAddress
	swapStatusByAddress       ActionStatusByAddress
	swapVolumesByPoolID       VolumesByPoolID
}

type ActionStatusByAddress map[string]schema.AccountActionStatus

func (m ActionStatusByAddress) ActionStatus(addr string) schema.AccountActionStatus {
	st, ok := m[addr]
	if !ok {
		st = schema.NewAccountActionStatus()
		m[addr] = st
	}
	return st
}

type VolumesByPoolID map[uint64]schema.Volumes

func (m VolumesByPoolID) Volumes(poolID uint64) schema.Volumes {
	v, ok := m[poolID]
	if !ok {
		v = make(schema.Volumes)
		m[poolID] = v
	}
	return v
}

func (t *Transformer) AccStateUpdates(ctx context.Context, startingBlockHeight int64) (*StateUpdates, error) {
	blockHeight := startingBlockHeight
	updates := &StateUpdates{
		depositStatusByAddress: make(ActionStatusByAddress),
		swapStatusByAddress:    make(ActionStatusByAddress),
		swapVolumesByPoolID:    make(VolumesByPoolID),
	}
	ignoredAddresses := t.cfg.IgnoredAddressesSet()
	for {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		default:
		}
		var data *BlockData
		var err error
		t.logger.Debug("waiting for the block data", zap.Int64("height", blockHeight))
		if blockHeight == startingBlockHeight {
			data, err = t.WaitForBlockData(ctx, blockHeight, 0)
			if err != nil {
				return nil, fmt.Errorf("wait for block data: %w", err)
			}
		} else {
			data, err = t.WaitForBlockData(ctx, blockHeight, t.cfg.BlockDataWaitingInterval+time.Second)
			if err != nil {
				if !errors.Is(err, context.DeadlineExceeded) {
					return nil, fmt.Errorf("wait for block data: %w", err)
				}
				break
			}
		}
		if data.Header.Height != blockHeight {
			return nil, fmt.Errorf("mismatching block height: expected %d, got %d", blockHeight, data.Header.Height)
		}
		updates.lastBlockData = data
		if data.BankModuleState != nil {
			updates.lastBankModuleState = data.BankModuleState
			updates.lastBankModuleStateHeight = blockHeight
		}
		tm := data.Header.Time.UTC()
		dateKey := tm.Format("2006-01-02")
		poolByID := data.PoolByID()
		t.logger.Debug("handling block data", zap.Int64("height", blockHeight), zap.Time("time", tm))
		for _, evt := range data.Events {
			switch evt.Type {
			case liquiditytypes.EventTypeDepositToPool:
				attrs := eventAttrsFromEvent(evt)
				addr, err := attrs.DepositorAddr()
				if err != nil {
					return nil, err
				}
				if _, ok := ignoredAddresses[addr]; ok {
					continue
				}
				poolID, err := attrs.PoolID()
				if err != nil {
					return nil, err
				}
				st := updates.depositStatusByAddress.ActionStatus(addr)
				st.IncreaseCount(poolID, dateKey, 1)
			case liquiditytypes.EventTypeSwapTransacted:
				attrs := eventAttrsFromEvent(evt)
				addr, err := attrs.SwapRequesterAddr()
				if err != nil {
					return nil, err
				}
				if _, ok := ignoredAddresses[addr]; ok {
					continue
				}
				poolID, err := attrs.PoolID()
				if err != nil {
					return nil, err
				}
				offerCoinFee, err := attrs.OfferCoinFee()
				if err != nil {
					return nil, err
				}
				swapPrice, err := attrs.SwapPrice()
				if err != nil {
					return nil, err
				}
				pool, ok := poolByID[poolID]
				if !ok {
					return nil, fmt.Errorf("pool id %d not found: %w", poolID, err)
				}
				demandCoinDenom, ok := oppositeReserveCoinDenom(pool, offerCoinFee.Denom)
				if !ok {
					return nil, fmt.Errorf("opposite reserve coin denom not found")
				}
				var demandCoinFee sdk.Coin
				if offerCoinFee.Denom < demandCoinDenom {
					demandCoinFee = sdk.NewCoin(demandCoinDenom, offerCoinFee.Amount.ToDec().Quo(swapPrice).TruncateInt())
				} else {
					demandCoinFee = sdk.NewCoin(demandCoinDenom, offerCoinFee.Amount.ToDec().Mul(swapPrice).TruncateInt())
				}
				st := updates.swapStatusByAddress.ActionStatus(addr)
				st.IncreaseCount(poolID, dateKey, 1)
				updates.swapVolumesByPoolID.Volumes(poolID).AddCoins(tm, schema.CoinMap{
					offerCoinFee.Denom:  offerCoinFee.Amount.Int64(),
					demandCoinFee.Denom: demandCoinFee.Amount.Int64(),
				})
			}
		}
		blockHeight++
	}
	return updates, nil
}
