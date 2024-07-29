/**
* This file was automatically generated by @oraichain/ts-codegen@0.35.9.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @oraichain/ts-codegen generate command to regenerate this file.
*/

import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult } from "@cosmjs/cosmwasm-stargate";
import { StdFee } from "@cosmjs/amino";
import {Uint128, Binary, Asset, Addr, Cw20ReceiveMsg, Coin, Cw20Coin, SwapOperation, IbcInfo, IbcFee, TransferBackMsg} from "./types";
import {InstantiateMsg, SwapVenue, ExecuteMsg, Action, Swap, Affiliate, SwapExactAssetOut, SwapExactAssetIn, SmartSwapExactAssetIn, Route, QueryMsg} from "./EntryPoint.types";
export interface EntryPointReadOnlyInterface {
  contractAddress: string;
  swapVenueAdapterContract: ({
    name
  }: {
    name: string;
  }) => Promise<Addr>;
  ibcTransferAdapterContract: () => Promise<Addr>;
}
export class EntryPointQueryClient implements EntryPointReadOnlyInterface {
  client: CosmWasmClient;
  contractAddress: string;

  constructor(client: CosmWasmClient, contractAddress: string) {
    this.client = client;
    this.contractAddress = contractAddress;
    this.swapVenueAdapterContract = this.swapVenueAdapterContract.bind(this);
    this.ibcTransferAdapterContract = this.ibcTransferAdapterContract.bind(this);
  }

  swapVenueAdapterContract = async ({
    name
  }: {
    name: string;
  }): Promise<Addr> => {
    return this.client.queryContractSmart(this.contractAddress, {
      swap_venue_adapter_contract: {
        name
      }
    });
  };
  ibcTransferAdapterContract = async (): Promise<Addr> => {
    return this.client.queryContractSmart(this.contractAddress, {
      ibc_transfer_adapter_contract: {}
    });
  };
}
export interface EntryPointInterface extends EntryPointReadOnlyInterface {
  contractAddress: string;
  sender: string;
  receive: ({
    amount,
    msg,
    sender
  }: {
    amount: Uint128;
    msg: Binary;
    sender: string;
  }, _fee?: number | StdFee | "auto", _memo?: string, _funds?: Coin[]) => Promise<ExecuteResult>;
  swapAndActionWithRecover: ({
    affiliates,
    minAsset,
    postSwapAction,
    recoveryAddr,
    sentAsset,
    timeoutTimestamp,
    userSwap
  }: {
    affiliates: Affiliate[];
    minAsset: Asset;
    postSwapAction: Action;
    recoveryAddr: Addr;
    sentAsset?: Asset;
    timeoutTimestamp: number;
    userSwap: Swap;
  }, _fee?: number | StdFee | "auto", _memo?: string, _funds?: Coin[]) => Promise<ExecuteResult>;
  swapAndAction: ({
    affiliates,
    minAsset,
    postSwapAction,
    sentAsset,
    timeoutTimestamp,
    userSwap
  }: {
    affiliates: Affiliate[];
    minAsset: Asset;
    postSwapAction: Action;
    sentAsset?: Asset;
    timeoutTimestamp: number;
    userSwap: Swap;
  }, _fee?: number | StdFee | "auto", _memo?: string, _funds?: Coin[]) => Promise<ExecuteResult>;
  userSwap: ({
    affiliates,
    minAsset,
    remainingAsset,
    swap
  }: {
    affiliates: Affiliate[];
    minAsset: Asset;
    remainingAsset: Asset;
    swap: Swap;
  }, _fee?: number | StdFee | "auto", _memo?: string, _funds?: Coin[]) => Promise<ExecuteResult>;
  postSwapAction: ({
    exactOut,
    minAsset,
    postSwapAction,
    timeoutTimestamp
  }: {
    exactOut: boolean;
    minAsset: Asset;
    postSwapAction: Action;
    timeoutTimestamp: number;
  }, _fee?: number | StdFee | "auto", _memo?: string, _funds?: Coin[]) => Promise<ExecuteResult>;
  updateConfig: ({
    ibcTransferContractAddress,
    ibcWasmContractAddress,
    owner,
    swapVenues
  }: {
    ibcTransferContractAddress?: string;
    ibcWasmContractAddress?: string;
    owner?: Addr;
    swapVenues?: SwapVenue[];
  }, _fee?: number | StdFee | "auto", _memo?: string, _funds?: Coin[]) => Promise<ExecuteResult>;
  universalSwap: ({
    memo
  }: {
    memo: string;
  }, _fee?: number | StdFee | "auto", _memo?: string, _funds?: Coin[]) => Promise<ExecuteResult>;
}
export class EntryPointClient extends EntryPointQueryClient implements EntryPointInterface {
  client: SigningCosmWasmClient;
  sender: string;
  contractAddress: string;

  constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string) {
    super(client, contractAddress);
    this.client = client;
    this.sender = sender;
    this.contractAddress = contractAddress;
    this.receive = this.receive.bind(this);
    this.swapAndActionWithRecover = this.swapAndActionWithRecover.bind(this);
    this.swapAndAction = this.swapAndAction.bind(this);
    this.userSwap = this.userSwap.bind(this);
    this.postSwapAction = this.postSwapAction.bind(this);
    this.updateConfig = this.updateConfig.bind(this);
    this.universalSwap = this.universalSwap.bind(this);
  }

  receive = async ({
    amount,
    msg,
    sender
  }: {
    amount: Uint128;
    msg: Binary;
    sender: string;
  }, _fee: number | StdFee | "auto" = "auto", _memo?: string, _funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      receive: {
        amount,
        msg,
        sender
      }
    }, _fee, _memo, _funds);
  };
  swapAndActionWithRecover = async ({
    affiliates,
    minAsset,
    postSwapAction,
    recoveryAddr,
    sentAsset,
    timeoutTimestamp,
    userSwap
  }: {
    affiliates: Affiliate[];
    minAsset: Asset;
    postSwapAction: Action;
    recoveryAddr: Addr;
    sentAsset?: Asset;
    timeoutTimestamp: number;
    userSwap: Swap;
  }, _fee: number | StdFee | "auto" = "auto", _memo?: string, _funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      swap_and_action_with_recover: {
        affiliates,
        min_asset: minAsset,
        post_swap_action: postSwapAction,
        recovery_addr: recoveryAddr,
        sent_asset: sentAsset,
        timeout_timestamp: timeoutTimestamp,
        user_swap: userSwap
      }
    }, _fee, _memo, _funds);
  };
  swapAndAction = async ({
    affiliates,
    minAsset,
    postSwapAction,
    sentAsset,
    timeoutTimestamp,
    userSwap
  }: {
    affiliates: Affiliate[];
    minAsset: Asset;
    postSwapAction: Action;
    sentAsset?: Asset;
    timeoutTimestamp: number;
    userSwap: Swap;
  }, _fee: number | StdFee | "auto" = "auto", _memo?: string, _funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      swap_and_action: {
        affiliates,
        min_asset: minAsset,
        post_swap_action: postSwapAction,
        sent_asset: sentAsset,
        timeout_timestamp: timeoutTimestamp,
        user_swap: userSwap
      }
    }, _fee, _memo, _funds);
  };
  userSwap = async ({
    affiliates,
    minAsset,
    remainingAsset,
    swap
  }: {
    affiliates: Affiliate[];
    minAsset: Asset;
    remainingAsset: Asset;
    swap: Swap;
  }, _fee: number | StdFee | "auto" = "auto", _memo?: string, _funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      user_swap: {
        affiliates,
        min_asset: minAsset,
        remaining_asset: remainingAsset,
        swap
      }
    }, _fee, _memo, _funds);
  };
  postSwapAction = async ({
    exactOut,
    minAsset,
    postSwapAction,
    timeoutTimestamp
  }: {
    exactOut: boolean;
    minAsset: Asset;
    postSwapAction: Action;
    timeoutTimestamp: number;
  }, _fee: number | StdFee | "auto" = "auto", _memo?: string, _funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      post_swap_action: {
        exact_out: exactOut,
        min_asset: minAsset,
        post_swap_action: postSwapAction,
        timeout_timestamp: timeoutTimestamp
      }
    }, _fee, _memo, _funds);
  };
  updateConfig = async ({
    ibcTransferContractAddress,
    ibcWasmContractAddress,
    owner,
    swapVenues
  }: {
    ibcTransferContractAddress?: string;
    ibcWasmContractAddress?: string;
    owner?: Addr;
    swapVenues?: SwapVenue[];
  }, _fee: number | StdFee | "auto" = "auto", _memo?: string, _funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      update_config: {
        ibc_transfer_contract_address: ibcTransferContractAddress,
        ibc_wasm_contract_address: ibcWasmContractAddress,
        owner,
        swap_venues: swapVenues
      }
    }, _fee, _memo, _funds);
  };
  universalSwap = async ({
    memo
  }: {
    memo: string;
  }, _fee: number | StdFee | "auto" = "auto", _memo?: string, _funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      universal_swap: {
        memo
      }
    }, _fee, _memo, _funds);
  };
}