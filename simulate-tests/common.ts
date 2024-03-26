import { SimulateCosmWasmClient } from "@oraichain/cw-simulate";
import { OraiswapTokenClient } from "@oraichain/oraidex-contracts-sdk";
// import { CwIcs20LatestClient } from "@oraichain/common-contracts-sdk";
import { CwIcs20LatestClient } from "./contracts-sdk/CwIcs20Latest.client";
import * as oraidexArtifacts from "@oraichain/oraidex-contracts-build";
import * as commonArtifacts from "@oraichain/common-contracts-build";
import { readFileSync } from "fs";
import { Cw20Coin } from "@oraichain/common-contracts-sdk";
import "dotenv/config";

export const TOKEN1 = "orai10ldgzued6zjp0mkqwsv2mux3ml50l97c74x8sg";
export const TOKEN2 = "orai1lus0f0rhx8s03gdllx2n6vhkmf0536dv57wfge";
export const TOKEN3 = "orai12hzjxfh77wl572gdzct2fxv2arxcwh6gykc7qh";
export const TOKEN4 =
  "orai15un8msx3n5zf9ahlxmfeqd2kwa5wm0nrpxer304m9nd5q6qq0g6sku5pdd";
export const senderAddress = "orai1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejvfgs7g";
export const bobAddress = "orai18cgmaec32hgmd8ls8w44hjn25qzjwhannd9kpj";

export const client = new SimulateCosmWasmClient({
  chainId: "Oraichain",
  bech32Prefix: "orai",
});

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
        },
        "token",
        "oraiswap_token"
      )
    ).contractAddress
  );
};

export const deployIcs20Token = async (
  client: SimulateCosmWasmClient,
  {
    swap_router_contract,
    converter_contract,
    gov_contract = senderAddress,
  }: {
    gov_contract?: string;
    swap_router_contract: string;
    converter_contract: string;
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
    },
    "cw-ics20-latest",
    "auto"
  );
  return new CwIcs20LatestClient(client, senderAddress, contractAddress);
};
