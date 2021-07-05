package main

import (
	"fmt"
	"strconv"

	sdk "github.com/cosmos/cosmos-sdk/types"
	liquiditytypes "github.com/tendermint/liquidity/x/liquidity/types"
	abcitypes "github.com/tendermint/tendermint/abci/types"
)

type DepositEvent struct {
	PoolID           uint64
	DepositorAddress string
	AcceptedCoins    sdk.Coins
}

func NewDepositEvent(event abcitypes.Event) (*DepositEvent, error) {
	attrs := EventAttributesFromEvent(event)
	poolID, err := attrs.PoolID()
	if err != nil {
		return nil, err
	}
	addr, err := attrs.DepositorAddress()
	if err != nil {
		return nil, err
	}
	coins, err := attrs.AcceptedCoins()
	if err != nil {
		return nil, err
	}
	return &DepositEvent{
		PoolID:           poolID,
		DepositorAddress: addr,
		AcceptedCoins:    coins,
	}, nil
}

type WithdrawEvent struct {
	PoolID            uint64
	WithdrawerAddress string
	WithdrawnCoins    sdk.Coins
}

func NewWithdrawEvent(event abcitypes.Event) (*WithdrawEvent, error) {
	attrs := EventAttributesFromEvent(event)
	poolID, err := attrs.PoolID()
	if err != nil {
		return nil, err
	}
	addr, err := attrs.WithdrawerAddress()
	if err != nil {
		return nil, err
	}
	coins, err := attrs.WithdrawnCoins()
	if err != nil {
		return nil, err
	}
	return &WithdrawEvent{
		PoolID:            poolID,
		WithdrawerAddress: addr,
		WithdrawnCoins:    coins,
	}, nil
}

type SwapEvent struct {
	PoolID               uint64
	SwapRequesterAddress string
	ExchangedOfferCoin   sdk.Coin
	ExchangedDemandCoin  sdk.Coin
}

func NewSwapEvent(event abcitypes.Event, poolByID map[uint64]liquiditytypes.Pool) (*SwapEvent, error) {
	attrs := EventAttributesFromEvent(event)
	poolID, err := attrs.PoolID()
	if err != nil {
		return nil, err
	}
	addr, err := attrs.SwapRequesterAddress()
	if err != nil {
		return nil, err
	}
	offerCoin, err := attrs.ExchangedOfferCoin()
	if err != nil {
		return nil, err
	}
	pool, ok := poolByID[poolID]
	if !ok {
		return nil, fmt.Errorf("pool %d not found", poolID)
	}
	demandCoinDenom, ok := OppositeReserveCoinDenom(pool, offerCoin.Denom)
	if !ok {
		return nil, fmt.Errorf("opposite reserve coin denom for %s in pool %d not found", offerCoin.Denom, poolID)
	}
	swapPrice, err := attrs.SwapPrice()
	if err != nil {
		return nil, err
	}
	var demandCoin sdk.Coin
	if offerCoin.Denom < demandCoinDenom {
		demandCoin = sdk.NewCoin(demandCoinDenom, offerCoin.Amount.ToDec().Quo(swapPrice).TruncateInt())
	} else {
		demandCoin = sdk.NewCoin(demandCoinDenom, offerCoin.Amount.ToDec().Mul(swapPrice).TruncateInt())
	}
	return &SwapEvent{
		PoolID:               poolID,
		SwapRequesterAddress: addr,
		ExchangedOfferCoin:   offerCoin,
		ExchangedDemandCoin:  demandCoin,
	}, nil
}

type EventAttributes map[string]string

func EventAttributesFromEvent(event abcitypes.Event) EventAttributes {
	m := make(EventAttributes)
	for _, attr := range event.Attributes {
		m[string(attr.Key)] = string(attr.Value)
	}
	return m
}

func (attrs EventAttributes) Attr(key string) (string, error) {
	v, ok := attrs[key]
	if !ok {
		return "", fmt.Errorf("attribute %q not found", key)
	}
	return v, nil
}

func (attrs EventAttributes) PoolID() (uint64, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValuePoolId)
	if err != nil {
		return 0, err
	}
	id, err := strconv.ParseUint(v, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("parse pool id: %w", err)
	}
	return id, nil
}

func (attrs EventAttributes) DepositorAddress() (string, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueDepositor)
	if err != nil {
		return "", err
	}
	return v, nil
}

func (attrs EventAttributes) AcceptedCoins() (sdk.Coins, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueAcceptedCoins)
	if err != nil {
		return nil, err
	}
	coins, err := sdk.ParseCoinsNormalized(v)
	if err != nil {
		return nil, fmt.Errorf("parse coins: %w", err)
	}
	return coins, nil
}

func (attrs EventAttributes) WithdrawerAddress() (string, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueWithdrawer)
	if err != nil {
		return "", err
	}
	return v, nil
}

func (attrs EventAttributes) WithdrawnCoins() (sdk.Coins, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueWithdrawCoins)
	if err != nil {
		return nil, err
	}
	coins, err := sdk.ParseCoinsNormalized(v)
	if err != nil {
		return nil, fmt.Errorf("parse coins: %w", err)
	}
	return coins, nil
}

func (attrs EventAttributes) SwapRequesterAddress() (string, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueSwapRequester)
	if err != nil {
		return "", err
	}
	return v, nil
}

func (attrs EventAttributes) ExchangedOfferCoin() (sdk.Coin, error) {
	denom, err := attrs.Attr(liquiditytypes.AttributeValueOfferCoinDenom)
	if err != nil {
		return sdk.Coin{}, err
	}
	v, err := attrs.Attr(liquiditytypes.AttributeValueExchangedOfferCoinAmount)
	if err != nil {
		return sdk.Coin{}, err
	}
	amt, ok := sdk.NewIntFromString(v)
	if !ok {
		return sdk.Coin{}, fmt.Errorf("parse offer coin amount: %q", v)
	}
	return sdk.NewCoin(denom, amt), nil
}

func (attrs EventAttributes) OfferCoinFee() (sdk.Coin, error) {
	denom, err := attrs.Attr(liquiditytypes.AttributeValueOfferCoinDenom)
	if err != nil {
		return sdk.Coin{}, err
	}
	v, err := attrs.Attr(liquiditytypes.AttributeValueOfferCoinFeeAmount)
	if err != nil {
		return sdk.Coin{}, err
	}
	amt, err := sdk.NewDecFromStr(v)
	if err != nil {
		return sdk.Coin{}, fmt.Errorf("parse offer coin fee amount: %w", err)
	}
	return sdk.NewCoin(denom, amt.TruncateInt()), nil
}

func (attrs EventAttributes) DemandCoinDenom() (string, error) {
	return attrs.Attr(liquiditytypes.AttributeValueDemandCoinDenom)
}

func (attrs EventAttributes) SwapPrice() (sdk.Dec, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueSwapPrice)
	if err != nil {
		return sdk.Dec{}, err
	}
	d, err := sdk.NewDecFromStr(v)
	if err != nil {
		return sdk.Dec{}, fmt.Errorf("parse swap price: %w", err)
	}
	return d, nil
}

func OppositeReserveCoinDenom(pool liquiditytypes.Pool, denom string) (string, bool) {
	for _, d := range pool.ReserveCoinDenoms {
		if d != denom {
			return d, true
		}
	}
	return "", false
}
