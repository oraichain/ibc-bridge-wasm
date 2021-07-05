package simulation

// DONTCOVER

import (
	"fmt"
	"math/rand"

	simtypes "github.com/cosmos/cosmos-sdk/types/simulation"
	"github.com/cosmos/cosmos-sdk/x/simulation"
	"github.com/cosmos/cosmos-sdk/codec"	
	cosmtypes "github.com/cosmos/cosmos-sdk/codec/types"
	"github.com/cosmos/cosmos-sdk/x/auth/legacy/legacytx"
	"github.com/cosmos/cosmos-sdk/client"
	"github.com/oraichain/orai/x/liquidity/types"
)

type EncodingConfig struct {
	InterfaceRegistry cosmtypes.InterfaceRegistry
	Marshaler         codec.Marshaler
	TxConfig          client.TxConfig
	Amino             *codec.LegacyAmino
}

// MakeTestEncodingConfig creates an EncodingConfig for an amino based test configuration.
// This function should be used only internally (in the SDK).
// App user shouldn't create new codecs - use the app.AppCodec instead.
// [DEPRECATED]
func MakeTestEncodingConfig() EncodingConfig {
	cdc := codec.NewLegacyAmino()
	interfaceRegistry := cosmtypes.NewInterfaceRegistry()
	marshaler := codec.NewAminoCodec(cdc)

	return EncodingConfig{
		InterfaceRegistry: interfaceRegistry,
		Marshaler:         marshaler,
		TxConfig:          legacytx.StdTxConfig{Cdc: cdc},
		Amino:             cdc,
	}
}

// ParamChanges defines the parameters that can be modified by param change proposals
// on the simulation
func ParamChanges(r *rand.Rand) []simtypes.ParamChange {
	return []simtypes.ParamChange{
		simulation.NewSimParamChange(types.ModuleName, string(types.KeyMinInitDepositAmount),
			func(r *rand.Rand) string {
				return fmt.Sprintf("\"%d\"", GenMinInitDepositAmount(r).Int64())
			},
		),
		simulation.NewSimParamChange(types.ModuleName, string(types.KeyInitPoolCoinMintAmount),
			func(r *rand.Rand) string {
				return fmt.Sprintf("\"%d\"", GenInitPoolCoinMintAmount(r).Int64())
			},
		),
		simulation.NewSimParamChange(types.ModuleName, string(types.KeyMaxReserveCoinAmount),
			func(r *rand.Rand) string {
				return fmt.Sprintf("\"%d\"", GenMaxReserveCoinAmount(r).Int64())
			},
		),
		simulation.NewSimParamChange(types.ModuleName, string(types.KeySwapFeeRate),
			func(r *rand.Rand) string {
				return fmt.Sprintf("\"%s\"", GenSwapFeeRate(r))
			},
		),
		simulation.NewSimParamChange(types.ModuleName, string(types.KeyWithdrawFeeRate),
			func(r *rand.Rand) string {
				return fmt.Sprintf("\"%s\"", GenWithdrawFeeRate(r))
			},
		),
		simulation.NewSimParamChange(types.ModuleName, string(types.KeyMaxOrderAmountRatio),
			func(r *rand.Rand) string {
				return fmt.Sprintf("\"%s\"", GenMaxOrderAmountRatio(r))
			},
		),
		simulation.NewSimParamChange(types.ModuleName, string(types.KeyUnitBatchHeight),
			func(r *rand.Rand) string {
				return fmt.Sprintf("%d", GenUnitBatchHeight(r))
			},
		),
	}
}
