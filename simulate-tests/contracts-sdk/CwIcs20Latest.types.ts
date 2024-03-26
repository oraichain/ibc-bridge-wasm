export interface InstantiateMsg {
  allowlist: AllowMsg[];
  converter_contract: string;
  default_gas_limit?: number | null;
  default_timeout: number;
  gov_contract: string;
  swap_router_contract: string;
}
export interface AllowMsg {
  contract: string;
  gas_limit?: number | null;
}
export type ExecuteMsg = {
  receive: Cw20ReceiveMsg;
} | {
  transfer_to_remote: TransferBackMsg;
} | {
  update_mapping_pair: UpdatePairMsg;
} | {
  delete_mapping_pair: DeletePairMsg;
} | {
  allow: AllowMsg;
} | {
  update_config: {
    admin?: string | null;
    converter_contract?: string | null;
    default_gas_limit?: number | null;
    default_timeout?: number | null;
    fee_denom?: string | null;
    fee_receiver?: string | null;
    relayer_fee?: RelayerFee[] | null;
    relayer_fee_receiver?: string | null;
    swap_router_contract?: string | null;
    token_fee?: TokenFee[] | null;
  };
} | {
  increase_channel_balance_ibc_receive: {
    amount: Uint128;
    dest_channel_id: string;
    ibc_denom: string;
    local_receiver: string;
  };
} | {
  reduce_channel_balance_ibc_receive: {
    amount: Uint128;
    ibc_denom: string;
    local_receiver: string;
    src_channel_id: string;
  };
} | {
  override_channel_balance: {
    channel_id: string;
    ibc_denom: string;
    outstanding: Uint128;
    total_sent?: Uint128 | null;
  };
} | {
  ibc_hooks_receive: {
    args: Binary;
    func: HookMethods;
  };
};
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
export type HookMethods = "universal_swap";
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
export interface UpdatePairMsg {
  denom: string;
  is_mint_burn?: boolean | null;
  local_asset_info: AssetInfo;
  local_asset_info_decimals: number;
  local_channel_id: string;
  remote_decimals: number;
}
export interface DeletePairMsg {
  denom: string;
  local_channel_id: string;
}
export interface RelayerFee {
  fee: Uint128;
  prefix: string;
}
export interface TokenFee {
  ratio: Ratio;
  token_denom: string;
}
export interface Ratio {
  denominator: number;
  nominator: number;
}
export type QueryMsg = {
  port: {};
} | {
  list_channels: {};
} | {
  channel: {
    id: string;
  };
} | {
  channel_with_key: {
    channel_id: string;
    denom: string;
  };
} | {
  config: {};
} | {
  admin: {};
} | {
  allowed: {
    contract: string;
  };
} | {
  list_allowed: {
    limit?: number | null;
    order?: number | null;
    start_after?: string | null;
  };
} | {
  pair_mappings: {
    limit?: number | null;
    order?: number | null;
    start_after?: string | null;
  };
} | {
  pair_mapping: {
    key: string;
  };
} | {
  pair_mappings_from_asset_info: {
    asset_info: AssetInfo;
  };
} | {
  get_transfer_token_fee: {
    remote_token_denom: string;
  };
};
export interface AdminResponse {
  admin?: string | null;
}
export interface AllowedResponse {
  gas_limit?: number | null;
  is_allowed: boolean;
}
export type Amount = {
  native: Coin;
} | {
  cw20: Cw20Coin;
};
export interface ChannelResponse {
  balances: Amount[];
  info: ChannelInfo;
  total_sent: Amount[];
}
export interface Coin {
  amount: Uint128;
  denom: string;
}
export interface Cw20Coin {
  address: string;
  amount: Uint128;
}
export interface ChannelInfo {
  connection_id: string;
  counterparty_endpoint: IbcEndpoint;
  id: string;
}
export interface IbcEndpoint {
  channel_id: string;
  port_id: string;
}
export interface ChannelWithKeyResponse {
  balance: Amount;
  info: ChannelInfo;
  total_sent: Amount;
}
export interface ConfigResponse {
  converter_contract: string;
  default_gas_limit?: number | null;
  default_timeout: number;
  fee_denom: string;
  gov_contract: string;
  relayer_fee_receiver: Addr;
  relayer_fees: RelayerFeeResponse[];
  swap_router_contract: string;
  token_fee_receiver: Addr;
  token_fees: TokenFee[];
}
export interface RelayerFeeResponse {
  amount: Uint128;
  prefix: string;
}
export interface ListAllowedResponse {
  allow: AllowedInfo[];
}
export interface AllowedInfo {
  contract: string;
  gas_limit?: number | null;
}
export interface ListChannelsResponse {
  channels: ChannelInfo[];
}
export interface PairQuery {
  key: string;
  pair_mapping: MappingMetadata;
}
export interface MappingMetadata {
  asset_info: AssetInfo;
  asset_info_decimals: number;
  is_mint_burn?: boolean;
  remote_decimals: number;
}
export type ArrayOfPairQuery = PairQuery[];
export interface PortResponse {
  port_id: string;
}