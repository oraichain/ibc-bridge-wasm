# Oraichain IBC Wasm Simulate tests

```bash
# Generate code and docs
cwtools build ../osor-api-contracts/contracts/entry-point ../osor-api-contracts/contracts/adapters/ibc/orai-ibc-wasm ../osor-api-contracts/contracts/adapters/swap/oraidex ../oraiswap/contracts/oraiswap_mixed_router ./contracts/* -o ./contracts/cw-ics20-latest/artifacts/

# gen schemas
cwtools build ../osor-api-contracts/contracts/entry-point ../osor-api-contracts/contracts/adapters/ibc/orai-ibc-wasm ../osor-api-contracts/contracts/adapters/swap/oraidex ../oraiswap/contracts/oraiswap_mixed_router ./contracts/* -o ./contracts/cw-ics20-latest/artifacts/ -s

# gen code:
cwtools gents ../osor-api-contracts/contracts/entry-point ../osor-api-contracts/contracts/adapters/ibc/orai-ibc-wasm ../osor-api-contracts/contracts/adapters/swap/oraidex ../oraiswap/contracts/oraiswap_mixed_router ./contracts/* -o simulate-tests/contracts-sdk/
```
