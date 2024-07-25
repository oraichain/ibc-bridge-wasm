import {Uint128, Binary, Addr, Cw20ReceiveMsg, Asset, Coin, Cw20Coin, Route} from "./types";
export interface InstantiateMsg {
  entry_point_contract_address: string;
}
export type ExecuteMsg = {
  receive: Cw20ReceiveMsg;
} | {
  swap: {
    operations: SwapOperation[];
  };
} | {
  transfer_funds_back: {
    return_denom: string;
    swapper: Addr;
  };
} | {
  astroport_pool_swap: {
    operation: SwapOperation;
  };
} | {
  white_whale_pool_swap: {
    operation: SwapOperation;
  };
};
export interface SwapOperation {
  denom_in: string;
  denom_out: string;
  interface?: Binary | null;
  pool: string;
}
export type QueryMsg = {
  simulate_swap_exact_asset_out: {
    asset_out: Asset;
    swap_operations: SwapOperation[];
  };
} | {
  simulate_swap_exact_asset_in: {
    asset_in: Asset;
    swap_operations: SwapOperation[];
  };
} | {
  simulate_swap_exact_asset_out_with_metadata: {
    asset_out: Asset;
    include_spot_price: boolean;
    swap_operations: SwapOperation[];
  };
} | {
  simulate_swap_exact_asset_in_with_metadata: {
    asset_in: Asset;
    include_spot_price: boolean;
    swap_operations: SwapOperation[];
  };
} | {
  simulate_smart_swap_exact_asset_in: {
    asset_in: Asset;
    routes: Route[];
  };
} | {
  simulate_smart_swap_exact_asset_in_with_metadata: {
    asset_in: Asset;
    include_spot_price: boolean;
    routes: Route[];
  };
};
export type Decimal = string;
export interface SimulateSmartSwapExactAssetInResponse {
  asset_out: Asset;
  spot_price?: Decimal | null;
}
export interface SimulateSwapExactAssetInResponse {
  asset_out: Asset;
  spot_price?: Decimal | null;
}
export interface SimulateSwapExactAssetOutResponse {
  asset_in: Asset;
  spot_price?: Decimal | null;
}