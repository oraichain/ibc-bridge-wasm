package main

import (
	liquiditytypes "github.com/tendermint/liquidity/x/liquidity/types"
	abcitypes "github.com/tendermint/tendermint/abci/types"
	tmproto "github.com/tendermint/tendermint/proto/tendermint/types"
)

type BlockData struct {
	Header tmproto.Header        `json:"block_header"`
	Events []abcitypes.Event     `json:"end_block_events"`
	Pools  []liquiditytypes.Pool `json:"pools"`
}

func (d *BlockData) PoolByID() map[uint64]liquiditytypes.Pool {
	m := make(map[uint64]liquiditytypes.Pool)
	for _, p := range d.Pools {
		m[p.Id] = p
	}
	return m
}
