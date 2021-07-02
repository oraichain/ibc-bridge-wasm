import Cosmos from '@oraichain/cosmosjs';
import { Buffer } from 'buffer';

const message = Cosmos.message;

const marketplaceContract = process.env.REACT_APP_MARKETPLACE_CONTRACT;
const nftContract = process.env.REACT_APP_NFT_CONTRACT;

/**
 * If there is chainId it will interacte with blockchain, otherwise using simulator
 */
class Wasm {
  constructor(network) {
    const chainId = network[0].toUpperCase() + network.substr(1);
    this.cosmos = new Cosmos(`https://lcd.${network}.orai.io`, chainId);
    this.cosmos.setBech32MainPrefix(network);
  }

  get contracts() {
    return {
      marketplaceContract,
      nftContract
    };
  }

  /**
   * query with json string
   * */
  async query(address, input) {
    const param = Buffer.from(input).toString('base64');
    return await this.cosmos.get(
      `/wasm/v1beta1/contract/${address}/smart/${param}`
    );
  }

  getHandleMessage(contract, msg, sender, funds, memo, denom) {
    const sent_funds = funds
      ? [{ denom: denom || this.cosmos.bech32MainPrefix, amount: funds }]
      : null;

    const msgSend = new message.cosmwasm.wasm.v1beta1.MsgExecuteContract({
      contract,
      msg,
      sender,
      sent_funds
    });

    const msgSendAny = new message.google.protobuf.Any({
      type_url: '/cosmwasm.wasm.v1beta1.MsgExecuteContract',
      value:
        message.cosmwasm.wasm.v1beta1.MsgExecuteContract.encode(
          msgSend
        ).finish()
    });

    return new message.cosmos.tx.v1beta1.TxBody({
      messages: [msgSendAny],
      memo
    });
  }

  async execute(
    address,
    input,
    childKey,
    { gas, fees, funds, memo, denom } = {}
  ) {
    const param = Buffer.from(input);
    const sender = this.getAddress(childKey);
    const txBody = this.getHandleMessage(
      address,
      param,
      sender,
      funds,
      memo,
      denom
    );
    return await this.cosmos.submit(
      childKey,
      txBody,
      'BROADCAST_MODE_BLOCK',
      fees || 0,
      gas || 200000
    );
  }

  async buyNft({ offeringId, amount, denom }, childKey) {
    const ret = await this.execute(
      marketplaceContract,
      JSON.stringify({
        buy_nft: {
          offering_id: parseInt(offeringId)
        }
      }),
      childKey,
      { funds: amount.toString(), denom } // need to have an option to specify gas because this transaction costs a lot of gas
    );
    return ret;
  }

  async sellNft({ description, image, name, tokenId, price }, childKey) {
    const ret = [];
    ret.push(
      await this.execute(
        marketplaceContract,
        JSON.stringify({
          mint_nft: {
            contract: nftContract,
            msg: btoa(
              JSON.stringify({
                mint: {
                  description,
                  image,
                  name,
                  owner: this.getAddress(childKey),
                  token_id: tokenId
                }
              })
            )
          }
        }),
        childKey
      )
    );

    ret.push(
      await this.execute(
        nftContract,
        JSON.stringify({
          send_nft: {
            contract: marketplaceContract,
            msg: btoa(
              JSON.stringify({
                price
              })
            ),
            token_id: tokenId
          }
        }),
        childKey
      )
    );
    return ret;
  }

  /**
   * get the public wallet address given a child key
   * @returns string
   */
  getAddress(childKey) {
    return this.cosmos.getAddress(childKey);
  }
}

export default Wasm;
