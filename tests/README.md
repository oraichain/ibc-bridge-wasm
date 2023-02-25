# E2E tests for IBC Bridge Wasm

## Steps to tests

1. Start the network by running: `docker-compose up -d` at the root of the repo

2. `yarn` to install Nodejs dependencies

3. Run `node deploy.js` to deploy necessary contracts & setup an IBC connection

4. Run `node transfer-cw20.js` using another tab to test transferring tokens