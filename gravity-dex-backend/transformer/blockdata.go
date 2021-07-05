package transformer

import (
	banktypes "github.com/cosmos/cosmos-sdk/x/bank/types"
	liquiditytypes "github.com/tendermint/liquidity/x/liquidity/types"
	abcitypes "github.com/tendermint/tendermint/abci/types"
	tmproto "github.com/tendermint/tendermint/proto/tendermint/types"
)

type BlockData struct {
	Header          tmproto.Header          `json:"block_header"`
	BankModuleState *banktypes.GenesisState `json:"bank_module_states"`
	Events          []abcitypes.Event       `json:"end_block_events"`
	Pools           []liquiditytypes.Pool   `json:"pools"`
}

func (d *BlockData) PoolByID() map[uint64]liquiditytypes.Pool {
	m := make(map[uint64]liquiditytypes.Pool)
	for _, p := range d.Pools {
		m[p.Id] = p
	}
	return m
}

func oppositeReserveCoinDenom(pool liquiditytypes.Pool, denom string) (string, bool) {
	for _, d := range pool.ReserveCoinDenoms {
		if d != denom {
			return d, true
		}
	}
	return "", false
}
