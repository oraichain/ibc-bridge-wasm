# Oraichain IBC Wasm Simulate tests

```bash
# Generate code and docs
cwtools build ../osor-api-contracts/contracts/entry-point ../osor-api-contracts/contracts/adapters/ibc/ibc-wasm ../osor-api-contracts/contracts/adapters/swap/oraidex ./contracts/* ../oraiswap/contracts/oraiswap_mixed_router -o ./contracts/cw-ics20-latest/artifacts/

# gen schemas
cwtools build ../osor-api-contracts/contracts/entry-point ../osor-api-contracts/contracts/adapters/ibc/ibc-wasm ../osor-api-contracts/contracts/adapters/swap/oraidex ./contracts/* ../oraiswap/contracts/oraiswap_mixed_router -o ./contracts/cw-ics20-latest/artifacts/ -s

# gen code:
cwtools gents ../oraiswap/contracts/_ ../oraidex-listing-contract ../co-harvest-contracts/contracts/_ ../cw20-staking/contracts/\* -o packages/contracts-sdk/src
```
