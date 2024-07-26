import {Asset, Uint128, Binary, Coin, Cw20Coin, TransferBackMsg, Cw20ReceiveMsg} from "./types";
export interface InstantiateMsg {
  entry_point_contract_address: string;
  ibc_wasm_contract_address: string;
}
export type ExecuteMsg = {
  ibc_wasm_transfer: {
    coin: Asset;
    ibc_wasm_info: TransferBackMsg;
  };
} | {
  receive: Cw20ReceiveMsg;
};
export type QueryMsg = string;