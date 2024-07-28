import {Uint128, Binary, SwapOperation, AssetInfo, Addr, Percentage, Cw20ReceiveMsg, PoolKey, FeeTier} from "./types";
export interface InstantiateMsg {
  entry_point_contract_address: string;
  oraidex_router_contract_address: string;
}
export type ExecuteMsg = {
  receive: Cw20ReceiveMsg;
} | {
  execute_swap_operations: {
    minimum_receive?: Uint128 | null;
    operations: SwapOperation[];
    to?: Addr | null;
  };
} | {
  execute_swap_operation: {
    operation: SwapOperation;
    sender: Addr;
    to?: Addr | null;
  };
} | {
  assert_minimum_receive_and_transfer: {
    asset_info: AssetInfo;
    minimum_receive: Uint128;
    receiver: Addr;
  };
} | {
  update_config: {
    factory_addr?: string | null;
    factory_addr_v2?: string | null;
    oraiswap_v3?: string | null;
    owner?: string | null;
  };
};
export type QueryMsg = {
  config: {};
} | {
  simulate_swap_operations: {
    offer_amount: Uint128;
    operations: SwapOperation[];
  };
};
export interface ConfigResponse {
  factory_addr: Addr;
  factory_addr_v2: Addr;
  oraiswap_v3: Addr;
}
export interface SimulateSwapOperationsResponse {
  amount: Uint128;
}