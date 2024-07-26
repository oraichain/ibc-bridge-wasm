export type Uint128 = string;
export type Binary = string;
export type AssetInfo = {
  token: {
    contract_addr: Addr;
  };
} | {
  native_token: {
    denom: string;
  };
};
export type Addr = string;
export interface Cw20ReceiveMsg {
  amount: Uint128;
  msg: Binary;
  sender: string;
}
export interface TransferBackMsg {
  local_channel_id: string;
  memo?: string | null;
  remote_address: string;
  remote_denom: string;
  timeout?: number | null;
}
export interface Coin {
  amount: Uint128;
  denom: string;
}
export type Asset = {
  native: Coin;
} | {
  cw20: Cw20Coin;
};
export interface Cw20Coin {
  address: string;
  amount: Uint128;
}
export type SwapOperation = {
  orai_swap: {
    ask_asset_info: AssetInfo;
    offer_asset_info: AssetInfo;
  };
} | {
  swap_v3: {
    pool_key: PoolKey;
    x_to_y: boolean;
  };
};
export type Percentage = number;
export interface PoolKey {
  fee_tier: FeeTier;
  token_x: string;
  token_y: string;
}
export interface FeeTier {
  fee: Percentage;
  tick_spacing: number;
}