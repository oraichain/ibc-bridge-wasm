import { SimulateCosmWasmClient } from "@oraichain/cw-simulate";
import { OraiswapTokenClient } from "@oraichain/oraidex-contracts-sdk";
// import { CwIcs20LatestClient } from "@oraichain/common-contracts-sdk";
import { CwIcs20LatestClient } from "./contracts-sdk/CwIcs20Latest.client";
import * as oraidexArtifacts from "@oraichain/oraidex-contracts-build";
import * as commonArtifacts from "@oraichain/common-contracts-build";
import { readFileSync } from "fs";
import { Cw20Coin } from "@oraichain/common-contracts-sdk";
import "dotenv/config";
import { InstantiateMsg as CwIcs20LatestInstantiateMsg } from "./contracts-sdk/CwIcs20Latest.types";
import { InstantiateMsg as OraiSwapTokenInstantiateMsg } from "@oraichain/oraidex-contracts-sdk/build/OraiswapToken.types";
import {
  EntryPointClient,
  OraidexClient,
  OraiIbcWasmClient,
  OraiswapMixedRouterClient,
} from "./contracts-sdk";
import { InstantiateMsg as OraiDexAdapterInstantiateMsg } from "./contracts-sdk/Oraidex.types";
import { InstantiateMsg as IbcWasmAdapterInstantiateMsg } from "./contracts-sdk/OraiIbcWasm.types";
import { InstantiateMsg as OsorEntrypointInstantiateMsg } from "./contracts-sdk/EntryPoint.types";
import { InstantiateMsg as MixedRouterInstantiateMsg } from "./contracts-sdk/OraiswapMixedRouter.types";

export const senderAddress = "orai1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejvfgs7g";

export const deployToken = async (
  client: SimulateCosmWasmClient,
  {
    symbol,
    name,
    decimals = 6,
    initial_balances = [{ address: senderAddress, amount: "1000000000" }],
    minter = senderAddress,
  }: {
    symbol: string;
    name: string;
    decimals?: number;
    initial_balances?: Cw20Coin[];
    minter?: string;
  }
): Promise<OraiswapTokenClient> => {
  return new OraiswapTokenClient(
    client,
    senderAddress,
    (
      await oraidexArtifacts.deployContract(
        client,
        senderAddress,

        {
          decimals,
          symbol,
          name,
          mint: { minter: minter },
          initial_balances,
        } as OraiSwapTokenInstantiateMsg,
        "token",
        "oraiswap-token"
      )
    ).contractAddress
  );
};

export const deployIbcWasmContract = async (
  client: SimulateCosmWasmClient,
  {
    swap_router_contract,
    converter_contract,
    gov_contract = senderAddress,
    osor_entrypoint_contract,
  }: {
    gov_contract?: string;
    swap_router_contract: string;
    converter_contract: string;
    osor_entrypoint_contract: string;
  }
): Promise<CwIcs20LatestClient> => {
  const { codeId } = await client.upload(
    senderAddress,
    readFileSync(
      process.env.ICS20_LATEST ||
        commonArtifacts.getContractDir("cw-ics20-latest")
    ),
    "auto"
  );
  const { contractAddress } = await client.instantiate(
    senderAddress,
    codeId,
    {
      allowlist: [],
      default_timeout: 3600,
      gov_contract,
      swap_router_contract,
      converter_contract,
      osor_entrypoint_contract,
    } as CwIcs20LatestInstantiateMsg,
    "cw-ics20-latest",
    "auto"
  );
  return new CwIcs20LatestClient(client, senderAddress, contractAddress);
};

export const deployOraiDexAdapterContract = async (
  client: SimulateCosmWasmClient,
  {
    osor_entrypoint_contract,
  }: {
    osor_entrypoint_contract: string;
  }
): Promise<OraidexClient> => {
  const { codeId } = await client.upload(
    senderAddress,
    readFileSync(process.env.ORAIDEX_ADAPTER),
    "auto"
  );
  const { contractAddress } = await client.instantiate(
    senderAddress,
    codeId,
    {
      entry_point_contract_address: osor_entrypoint_contract,
    } as OraiDexAdapterInstantiateMsg,
    "oraidex-adapter",
    "auto"
  );
  return new OraidexClient(client, senderAddress, contractAddress);
};

export const deployIbcWasmAdapterContract = async (
  client: SimulateCosmWasmClient,
  {
    osor_entrypoint_contract,
  }: {
    osor_entrypoint_contract: string;
  }
): Promise<OraiIbcWasmClient> => {
  const { codeId } = await client.upload(
    senderAddress,
    readFileSync(process.env.IBC_WASM_ADAPTER),
    "auto"
  );
  const { contractAddress } = await client.instantiate(
    senderAddress,
    codeId,
    {
      entry_point_contract_address: osor_entrypoint_contract,
    } as IbcWasmAdapterInstantiateMsg,
    "ibc-wasm-adapter",
    "auto"
  );
  return new OraiIbcWasmClient(client, senderAddress, contractAddress);
};

export const deployOsorEntrypointContract = async (
  client: SimulateCosmWasmClient,
  {
    ibc_wasm_contract_address,
  }: {
    ibc_wasm_contract_address: string;
  }
): Promise<EntryPointClient> => {
  const { codeId } = await client.upload(
    senderAddress,
    readFileSync(process.env.OSOR_ENTRYPOINT),
    "auto"
  );
  const { contractAddress } = await client.instantiate(
    senderAddress,
    codeId,
    {
      ibc_wasm_contract_address,
      swap_venues: [],
    } as OsorEntrypointInstantiateMsg,
    "osor-entrypoint",
    "auto"
  );
  return new EntryPointClient(client, senderAddress, contractAddress);
};

export const deployMixedRouterContract = async (
  client: SimulateCosmWasmClient,
  {
    factory_addr,
    oraiswap_v3,
  }: {
    factory_addr: string;
    oraiswap_v3: string;
  }
): Promise<OraiswapMixedRouterClient> => {
  const { codeId } = await client.upload(
    senderAddress,
    readFileSync(process.env.MIXED_ROUTER),
    "auto"
  );
  const { contractAddress } = await client.instantiate(
    senderAddress,
    codeId,
    {
      factory_addr,
      factory_addr_v2: factory_addr,
      oraiswap_v3,
    } as MixedRouterInstantiateMsg,
    "mixed-router",
    "auto"
  );
  return new OraiswapMixedRouterClient(client, senderAddress, contractAddress);
};
