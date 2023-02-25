const cp = require('child_process');
const { execSync } = require('child_process');

/**
 * 
 * @param {*} network - can be mars or earth 
 * @param {*} address - address to query
 * @param {*} denom - native coin denom
 * @returns 
 */
function queryNativeBalance(network, address, denom) {
    return parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T ${network} ash -c 'oraid query bank balances ${address} --output json --denom ${denom}'`))).amount);
}

/**
 * 
 * @param {*} network - can be mars or earth 
 * @param {*} address - contract address to query state
 * @param {*} msg - js object of the msg
 * @returns 
 */
function queryContractState(network, address, msg) {
    return JSON.parse(Buffer.from(execSync(`docker-compose exec -T ${network} ash -c 'oraid query wasm contract-state smart ${address} ${msg} --output json'`)));
}

/**
 * 
 * @param {*} network - can be mars or earth 
 * @param {*} address - contract address to execute
 * @param {*} msg - js object of the msg
 * @returns 
 */
function executeContract(network, address, msg) {
    return JSON.parse(Buffer.from(execSync(`docker-compose exec -T ${network} ash -c 'oraid tx wasm execute ${address} ${msg} --from $USER --chain-id $CHAIN_ID -y -b block --keyring-backend test --gas 20000000 --output json'`)));
}

function spawnHermes(background = false) {
    let args = ["exec", "-T", "hermes", "bash", "-c", "hermes --config config.toml start"];
    if (background) args = ["exec", "-d", "-T", "hermes", "bash", "-c", "hermes --config config.toml start"];

    const hermesSpawn = cp.spawn('docker-compose', args);
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

module.exports = {
    queryNativeBalance,
    queryContractState,
    executeContract,
    spawnHermes
}