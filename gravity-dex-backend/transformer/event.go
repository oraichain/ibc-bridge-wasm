package transformer

import (
	"fmt"
	"strconv"

	sdk "github.com/cosmos/cosmos-sdk/types"
	liquiditytypes "github.com/tendermint/liquidity/x/liquidity/types"
	abcitypes "github.com/tendermint/tendermint/abci/types"
)

type EventAttributes map[string]string

func eventAttrsFromEvent(event abcitypes.Event) EventAttributes {
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

func (attrs EventAttributes) DepositorAddr() (string, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueDepositor)
	if err != nil {
		return "", err
	}
	return v, nil
}

func (attrs EventAttributes) SwapRequesterAddr() (string, error) {
	v, err := attrs.Attr(liquiditytypes.AttributeValueSwapRequester)
	if err != nil {
		return "", err
	}
	return v, nil
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
