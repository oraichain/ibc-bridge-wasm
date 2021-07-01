import Cosmos from '@oraichain/cosmosjs';
import { Buffer } from 'buffer';

const message = Cosmos.message;

/**
 * If there is chainId it will interacte with blockchain, otherwise using simulator
 */
class Wasm {
  constructor({ network, mnemonic }) {
    const chainId = network[0].toUpperCase() + network.substr(1);
    this.cosmos = new Cosmos(`http://lcd.${network}`, chainId);
    this.cosmos.setBech32MainPrefix(network);
    this.childKey = this.cosmos.getChildKey(mnemonic);
  }

  /**
   * query with json string
   * */
  async query(address, input) {
    const param = Buffer.from(input);
    return await this.cosmos.get(
      `/wasm/v1beta1/contract/${address}/smart/${param}`
    );
  }

  getHandleMessage(contract, msg, sender, funds, memo) {
    const sent_funds = funds
      ? [{ denom: this.cosmos.bech32MainPrefix, amount: funds }]
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

  async execute(address, input, { gas, fees, funds, memo } = {}) {
    const param = Buffer.from(input);
    const sender = this.getAddress(this.childKey);
    const txBody = this.getHandleMessage(address, param, sender, funds, memo);
    return await this.cosmos.submit(
      this.childKey,
      txBody,
      'BROADCAST_MODE_BLOCK',
      fees || 0,
      gas || 200000
    );
  }

  async mintNft(
    marketplaceContract,
    nftContract,
    { description, image, name, tokenId }
  ) {
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
                owner: this.getAddress(),
                token_id: tokenId
              }
            })
          )
        }
      })
    );
  }

  /**
   * get the public wallet address given a child key
   * @returns string
   */
  getAddress(childKey) {
    return this.cosmos.getAddress(childKey || this.childKey);
  }
}

export default Wasm;
