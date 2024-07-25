import {Uint128, Coin, IbcInfo, IbcFee} from "./types";
export interface InstantiateMsg {
  entry_point_contract_address: string;
}
export type ExecuteMsg = {
  ibc_transfer: {
    coin: Coin;
    info: IbcInfo;
    timeout_timestamp: number;
  };
};
export type QueryMsg = {
  in_progress_recover_address: {
    channel_id: string;
    sequence_id: number;
  };
};
export type String = string;