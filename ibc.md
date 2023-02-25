# IBC transfer flow:

Let's assume the network that has the contract cw20-ics20 deployed is network B, the other network is A.
In the cw-ics20-latest contract, there are couple transfer flows in the code below:

## Network A transfers native tokens to B first (A->B, where native token is not IBC token)

In this case, the packet is caught in the `do_ibc_packet_receive` function, we increase the balance's channel then forward the packet to the `allow_contract` contract for minting. Then, the cw-ics20 contract returns an acknowledgement to the network A.

Here, if any line returns an error, then the cw-ics20 contract will send a fail acknowledgement to the network A, and the network A will refund the tokens to the sender according to the `ibctransfer` application.

## Network A transfers IBC tokens to B (A->B, where IBC tokens are tokens that were previously sent from B to A)

In this case, the packet is also caught in the `do_ibc_packet_receive`, but we parse the ibc denom to get the original denom instead, and send the same amount that is stored in the cw-ics20 contract to the receiver using SubMsg (with reply on error). This is the logic that the CosmWasm developers wrote.

## Network B transfers tokens to A (B->A, where tokens are either native or cw20 that originated from B)

In this case, the user will deposit his tokens into the cw-ics20 contract, then the contract will create a new IBCPacket and transfer it to the network A. The cw-ics20 contract then invokes the `ibc_packet_ack` function, which has the acknowledgement packet sent from A. If the ack is a failure, then we refund (using SubMsg with reply on error) the original sender by using the tokens stored on the cw-ics20 contract. This is also the logic that CosmWasm developers wrote.

## Network B transfers tokens to A (B->A, where tokens are native tokens from A)

In this case, the user will deposit tokens to the `allow_contract`, and the allow contract will call the `execute_transfer_back_to_remote_chain` function. Here, the cw-ics20 contract will not hold any tokens. Instead, it only forwards the logic to the chain A. The cw-ics20 contract then invokes the `ibc_packet_ack` function, which has the acknowledgement packet sent from A. If the ack is a failure, then we refund by calling a message to the `allow_contract` requesting for a refund. We use the `CosmosMsg` instead of the `SubMsg`. 

This is really important because by using the CosmosMsg, we force the `allow_contract` to actually refunds successfully. If we use SubMsg, then it could be that the `allow_contract` fails somewhere, and only its state gets reverted, aka the sender receives no refunds.

If we use CosmosMsg, then the acknowledgement packet will fail entirely, and it will be retried by the relayer as long as we fix the `allow_contract`.

Normally, if it is a `ibctransfer` application developed as a submodule in Cosmos SDK, then the refund part must not fail, and we can trust that it will not fail. However, the `allow_contract` can be developed by anyone, and can be replaced => cannot be trusted.