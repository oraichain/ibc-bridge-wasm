const fs = require('fs');
const path = require("path");

const cp = require('child_process');
const assert = require('assert');

// We have this address's key, so it is used to create txs 
const mainMarsAddress = "mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq";

/**
 * this function takes a deploy command and run it, then extract the contract address from the output 
 * @param {*} deployCommand 
 * @returns contract address
 */
function deployAndGetAddress(deployCommand) {
    const deployCwIcs20Result = Buffer.from(cp.execSync(deployCommand)).toString('ascii');
    // parse the exec result to get the newly deployed contract address. this address should be used in the next commands
    const searchString = "contract address: ";
    const contractAddress = deployCwIcs20Result.substring(deployCwIcs20Result.indexOf(searchString) + searchString.length).trim();
    console.log("contract address is: ", contractAddress)
    return contractAddress;
}

/**
 * get channel id from the output of creating a new connection using hermes command
 * @param {*} createChannelResult - a string including the new channel created
 */
function parseChannelId(createChannelResult) {
    return createChannelResult.match(/channel-\d+/)[0];
}

function spawnHermes() {
    const hermesSpawn = cp.spawn('docker-compose', ["exec", "-T", "hermes", "bash", "-c", "hermes --config config.toml start"]);
    hermesSpawn.stdout.on('data', (data) => {
        console.log(`stdout: ${data}`);
    });

    hermesSpawn.stderr.on('data', (data) => {
        console.error(`stderr: ${data}`);
    });

    hermesSpawn.on('close', (code) => {
        console.log(`child process exited with code ${code}`);
    });
}

/**
 * 
 * @param {*} msg - msg object 
 * @returns 
 */
function parseDockerMessage(msg) {
    return JSON.stringify(JSON.stringify(msg));
}

async function start() {

    const rootDir = path.dirname(__dirname);

    try {
        // build latest cw-ics20 contract
        const buildResult = Buffer.from(cp.execSync(`${rootDir}/scripts/build_contract.sh ${rootDir}/contracts/cw-ics20-latest`)).toString('ascii');
        console.log("buildResult: ", buildResult);

        // copy to mars dir cw-ics20 to create a new cw-ics20
        let copyWasmResult = Buffer.from(cp.execSync(`sudo cp ${rootDir}/contracts/cw-ics20-latest/artifacts/cw-ics20-latest.wasm ${rootDir}/.mars`)).toString('ascii');
        console.log("copy cw-ics20 wasm result: ", copyWasmResult);

        // copy to mars dir cw20 contract to create new cw20
        copyWasmResult = Buffer.from(cp.execSync(`sudo cp ${rootDir}/contracts/cw20-base/artifacts/cw20-base.wasm ${rootDir}/.mars`)).toString('ascii');
        console.log("copy cw20 wasm result: ", copyWasmResult);

        // deploy cw ics20. -T flag is used to fix error: input device is not a tty. Ref: https://stackoverflow.com/questions/43099116/error-the-input-device-is-not-a-tty
        const cwIcs20Address = deployAndGetAddress(`docker-compose exec -T mars ash -c './scripts/deploy_contract.sh .mars/cw-ics20-latest.wasm "cw20-ics20-latest" ${parseDockerMessage({ "default_timeout": 180, "gov_contract": mainMarsAddress, "allowlist": [] })}'`);

        // after deploy the ics20 address, the address must not be empty
        assert.notStrictEqual(cwIcs20Address, "");

        // deploy cw20 tokens
        const cw20Address = deployAndGetAddress(`docker-compose exec -T mars ash -c './scripts/deploy_contract.sh .mars/cw20-base.wasm "cw20-base" ${parseDockerMessage({ "name": "EARTH", "symbol": "EARTH", "decimals": 6, "initial_balances": [{ "address": cwIcs20Address, "amount": "100" }], "mint": { "minter": mainMarsAddress } })}'`);

        // after deploy the ics20 address, the address must not be empty
        assert.notStrictEqual(cw20Address, "");

        // init hermes accounts
        try {
            const addAccounts = Buffer.from(cp.execSync(`docker-compose exec -T hermes bash -c 'hermes --config config.toml keys add --chain Earth --mnemonic-file accounts/Earth.txt && hermes --config config.toml keys add --chain Mars --mnemonic-file accounts/Mars.txt'`)).toString('ascii');
            console.log("add accounts hermes: ", addAccounts);
        } catch (error) {
            console.log("We have added keys already. Skip this step ...");
        }

        // init new channels with port as the cwics20 address
        const hermesCreateChannelResult = Buffer.from(cp.execSync(`docker-compose exec -T hermes bash -c 'hermes --config config.toml create channel --a-chain Earth --b-chain Mars --a-port transfer --b-port wasm.${cwIcs20Address} --new-client-connection --yes'`)).toString('ascii');
        console.log("hermes create channel result: ", hermesCreateChannelResult);
        const channelId = parseChannelId(hermesCreateChannelResult);
        console.log("hermes new channel: ", channelId);

        // create new cw20 mapping pair
        let updateNewMappingPairResult = JSON.parse(Buffer.from(cp.execSync(`docker-compose exec -T mars ash -c 'oraid tx wasm execute ${cwIcs20Address} ${parseDockerMessage({ "update_mapping_pair": { "local_channel_id": channelId, "denom": "uusd", "asset_info": { "token": { "contract_addr": cw20Address } }, "remote_decimals": 6, "asset_info_decimals": 6 } })} -y --from $USER --chain-id $CHAIN_ID --keyring-backend test -b block --output json'`)));
        console.log("update new mapping pair result: ", updateNewMappingPairResult);

        // the update mapping pair tx must succeed before we can do anything else
        assert.deepEqual(updateNewMappingPairResult.code, 0);

        // create new native mapping pair
        updateNewMappingPairResult = JSON.parse(Buffer.from(cp.execSync(`docker-compose exec -T mars ash -c 'oraid tx wasm execute ${cwIcs20Address} ${parseDockerMessage({ "update_mapping_pair": { "local_channel_id": channelId, "denom": "earth", "asset_info": { "native_token": { "denom": "mars" } }, "remote_decimals": 6, "asset_info_decimals": 6 } })} -y --from $USER --chain-id $CHAIN_ID --keyring-backend test -b block --output json'`)));
        console.log("update new mapping pair result: ", updateNewMappingPairResult);

        // the update mapping pair tx must succeed before we can do anything else
        assert.deepEqual(updateNewMappingPairResult.code, 0);

        // write all deployed data to a json file so that other tests can read and use.
        fs.writeFile(path.join(__dirname, 'test-data.json'), JSON.stringify({ cwIcs20Address, cw20Address, channelId }), (err) => {
            if (err) throw err;
        });

        // start hermes
        spawnHermes();

    } catch (error) {
        console.log("error when running the script: ", error);
    }

}

start();