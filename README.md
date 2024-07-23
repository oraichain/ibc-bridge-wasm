# IBC transfer flow:

Let's assume the network that has the contract cw20-ics20 deployed is network B, the other network is A.

In the source code, we call the network having cw20-ics20 deployed local chain, other networks are remote chains.

As of now, we only support A bridging to B first, not the other way around.

In the cw-ics20-latest contract, there are couple transfer flows in the code below:

## Network A transfers native tokens to B first (A->B, where native token is not IBC token)

In this case, the packet is caught in the `do_ibc_packet_receive` function, we increase the balance's channel then forward the packet to the `allow_contract` contract for minting. Then, the cw-ics20 contract returns an acknowledgement to the network A.

Here, if any line returns an error, then the cw-ics20 contract will send a fail acknowledgement to the network A, and the network A will refund the tokens to the sender according to the `ibctransfer` application.

## Network A transfers IBC tokens to B (A->B, where IBC tokens are tokens that were previously sent from B to A)

Currently not supported.

## Network B transfers tokens to A (B->A, where tokens are either native or cw20 that originated from B)

Currently not supported.

## Network B transfers tokens to A (B->A, where tokens are native tokens from A)

In this case, the user will deposit tokens and invoke the function `execute_transfer_back_to_remote_chain` function. It will create an IBC Send packet and lock the cw20 or native token in the cw20-ics20 contract. If there's a failed acknowledgement, the contract will try to automatically refund the locked tokens back to the sender. If the refund msg is failed, then the token will be locked in the contract, and the team will refund these tokens manually for security purposes.

## Protobuf install

```bash
# macos
brew install protobuf

cargo install protoc-gen-prost

protoc --prost_out packages/cw20-ics20-msg/src/ -I proto proto/universal-swap-memo.proto && mv packages/cw20-ics20-msg/src/_ packages/cw20-ics20-msg/src/universal_swap_memo.rs
```
