const fs = require('fs');
const path = require("path");

const { execSync } = require('child_process');
const assert = require('assert');
const { queryNativeBalance, queryContractState, executeContract, spawnHermes } = require('./utils');

// We have this address's key, so it is used to create txs 
const mainMarsAddress = "mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq";
const mainEarthAddress = "earth1w84gt7t7dzvj6qmf5q73d2yzyz35uwc7y8fkwp";
const networks = {
    EARTH: "earth",
    MARS: "mars"
}

async function start() {

    // should in form: {"cwIcs20Address":"mars1fventeva948ue0fzhp6xselr522rnqwger9wg7r0g9f4jemsqh6seanjnj","cw20Address":"mars18yn206ypuxay79gjqv6msvd9t2y49w4fz8q7fyenx5aggj0ua37q9sm4kk","channelId":"channel-5"}
    const testData = JSON.parse(fs.readFileSync(path.join(__dirname, 'test-data.json')));

    try {

        console.log("Test send from remote chain to local chain");

        // console.log("test ibc transfer to cw20 success");
        await testIbcTransferCw20SuccessShouldIncreaseCw20BalanceRemoteToLocal(testData);

        console.log("test ibc transfer to cw20 failed no mapping should refund");
        await testIbcTransferCw20FailNoPairMappingShouldRefundRemoteToLocal(testData);

        console.log("test ibc transfer failed insufficient funds");
        await testIbcTransferCw20FailInsufficientFundsShouldRefundRemoteToLocal(testData);

        console.log("test ibc transfer native success should increase fund");
        await testIbcTransferNativeSuccessShouldIncreaseNativeBalanceRemoteToLocal(testData);

        console.log("test send from local chain to remote chain");
        console.log("test ibc transfer to cw20 failed outcoming channel balance > incoming channel balance");
        await testIbcTransferCw20FailOutComingChannelLargerIncomingShouldRefundLocalToRemote(testData);

        console.log("test ibc transfer from cw20 to native success");
        await testIbcTransferCw20SuccessShouldIncreaseNativeBalanceRemoteToLocal(testData);

        console.log("test transfer native fail out channel balance larger than in channel balance should refund");
        await testIbcTransferNativeFailOutChannelBalanceLargerThanInChannelBalanceShouldRefundTokensLocalToRemote(testData);

        console.log("test done!");

    } catch (error) {
        console.log("error when running the script: ", error);
    }

}

/**
 * 
 * @param {*} msg - msg object 
 * @returns 
 */
function parseDockerMessage(msg) {
    return JSON.stringify(JSON.stringify(msg));
}

async function sleep(sleepTime = 20000) {
    console.log("Taking a nap ...");
    return await new Promise((resolve) => setTimeout(resolve, sleepTime)); // 20000ms = 20s = 4 blocks, should finish IBC transactions by now
}

async function testIbcTransferCw20SuccessShouldIncreaseCw20BalanceRemoteToLocal(testData) {

    const coin = {
        amount: 1,
        denom: "uusd"
    }

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));
    assert.deepEqual(ibcTransferResult.code, 0);

    // query balance before receiving new tokens
    const balanceBefore = parseInt(queryContractState(networks.MARS, testData.cw20Address, parseDockerMessage({ "balance": { "address": mainMarsAddress } })).data.balance);
    // query channel balance before
    let channelBalanceBefore = queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id": testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom));;
    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    const balanceAfter = parseInt(queryContractState(networks.MARS, testData.cw20Address, parseDockerMessage({ "balance": { "address": mainMarsAddress } })).data.balance);
    // channel balance after
    const channelBalanceAfter = parseInt(queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom)).native.amount);
    assert.deepEqual(balanceBefore + coin.amount, balanceAfter);
    assert.deepEqual(channelBalanceBefore + coin.amount, channelBalanceAfter);
}

async function testIbcTransferCw20FailNoPairMappingShouldRefundRemoteToLocal(testData) {

    const coin = {
        amount: 1,
        denom: "uatom"
    }

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));

    assert.deepEqual(ibcTransferResult.code, 0);

    // query channel balance before
    let channelBalanceBefore = queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom));

    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    // channel balance after
    let channelBalanceAfter = queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom));
    channelBalanceAfter = channelBalanceAfter ? parseInt(channelBalanceAfter.native.amount) : 0;

    assert.deepEqual(channelBalanceBefore, channelBalanceAfter);
}

async function testIbcTransferCw20FailInsufficientFundsShouldRefundRemoteToLocal(testData) {

    const coin = {
        amount: 1000,
        denom: "uusd"
    }

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));

    assert.deepEqual(ibcTransferResult.code, 0);

    // query balance before receiving new tokens
    const balanceBefore = parseInt(queryContractState(networks.MARS, testData.cw20Address, parseDockerMessage({ "balance": { "address": mainMarsAddress } })).data.balance);

    // query channel balance before
    let channelBalanceBefore = queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom));

    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    const balanceAfter = parseInt(queryContractState(networks.MARS, testData.cw20Address, parseDockerMessage({ "balance": { "address": mainMarsAddress } })).data.balance);

    // channel balance after
    const channelBalanceAfter = parseInt(queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom)).native.amount);
    assert.deepEqual(balanceBefore, balanceAfter);
    assert.deepEqual(channelBalanceBefore, channelBalanceAfter);
}

async function testIbcTransferNativeSuccessShouldIncreaseNativeBalanceRemoteToLocal(testData) {

    const coin = {
        amount: 1,
        denom: "earth"
    }

    // send native mars token to ibc wasm contract first
    const nativeTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid tx send $USER ${testData.cwIcs20Address} ${coin.amount}mars --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));
    assert.deepEqual(nativeTransferResult.code, 0);

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));
    assert.deepEqual(ibcTransferResult.code, 0);

    // query mars balance before receiving new tokens
    const balanceBefore = queryNativeBalance(networks.MARS, mainMarsAddress, networks.MARS);
    // query channel balance before
    let channelBalanceBefore = queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom));
    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    const balanceAfter = queryNativeBalance(networks.MARS, mainMarsAddress, networks.MARS);
    // channel balance after
    const channelBalanceAfter = parseInt(queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom)).native.amount);
    assert.deepEqual(balanceBefore + coin.amount, balanceAfter);
    assert.deepEqual(channelBalanceBefore + coin.amount, channelBalanceAfter);
}

async function testIbcTransferCw20FailOutComingChannelLargerIncomingShouldRefundLocalToRemote(testData) {

    const coin = {
        amount: 1,
        denom: "uusd"
    }

    // query channel balance before
    let channelBalanceBefore = queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom));
    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    const cw20Msg = JSON.stringify({ "local_channel_id": testData.channelId, "remote_address": mainEarthAddress, "remote_denom": coin.denom });
    // always transfer over the channel balance to get error
    const transferAmount = (channelBalanceBefore + 1).toString();
    const transferBackResult = executeContract(networks.MARS, testData.cw20Address, parseDockerMessage({ "send": { "amount": transferAmount, "contract": testData.cwIcs20Address, "msg": Buffer.from(cw20Msg).toString('base64') } }));
    // code should be 5, overflow
    assert.deepEqual(transferBackResult.code, 5);
    assert.deepEqual(transferBackResult.raw_log.includes('Overflow'), true);
}

async function testIbcTransferCw20SuccessShouldIncreaseNativeBalanceRemoteToLocal(testData) {

    const coin = {
        amount: 1,
        denom: "uusd"
    }

    // query mars balance before receiving new tokens
    const balanceInitial = queryNativeBalance(networks.EARTH, mainEarthAddress, coin.denom);

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));
    assert.deepEqual(ibcTransferResult.code, 0);

    // wait for relayer to relay from remote to local
    await sleep();

    // query mars balance before receiving new tokens
    const balanceBefore = queryNativeBalance(networks.EARTH, mainEarthAddress, coin.denom);
    assert.deepEqual(balanceBefore, balanceInitial - 1);

    // after transferring to local chain, we try to send back to remote chain. Should pass and increase balance in the remote chain
    const cw20Msg = JSON.stringify({ "local_channel_id": testData.channelId, "remote_address": mainEarthAddress, "remote_denom": coin.denom });
    const transferBackResult = executeContract(networks.MARS, testData.cw20Address, parseDockerMessage({ "send": { "amount": coin.amount.toString(), "contract": testData.cwIcs20Address, "msg": Buffer.from(cw20Msg).toString('base64') } }));
    console.log("transfer back result: ", transferBackResult);
    // code should be 0, success
    assert.deepEqual(transferBackResult.code, 0);

    // we sleep and wait for the relayer to do its job
    await sleep();

    const balanceAfter = queryNativeBalance(networks.EARTH, mainEarthAddress, coin.denom);
    assert.deepEqual(balanceAfter, balanceBefore);
}

async function testIbcTransferNativeFailOutChannelBalanceLargerThanInChannelBalanceShouldRefundTokensLocalToRemote(testData) {

    const coin = {
        amount: 1,
        denom: networks.EARTH
    }
    // first we need to transfer from remote to local successfully, then we transfer back to remote to test
    await testIbcTransferNativeSuccessShouldIncreaseNativeBalanceRemoteToLocal(testData);
    await sleep(5000); // sleep 5 sec before creating a new tx so that it does not get account sequence error
    // query mars balance before transferring back to remote chain
    const balanceBefore = queryNativeBalance(networks.MARS, mainMarsAddress, networks.MARS);

    const transferBackMsg = JSON.parse(execSync(`docker-compose exec -T mars ash -c 'oraid tx wasm execute ${testData.cwIcs20Address} ${parseDockerMessage({ "transfer_to_remote": { "local_channel_id": testData.channelId, "remote_address": mainEarthAddress, "remote_denom": coin.denom } })} --from duc --chain-id $CHAIN_ID -y -b block --keyring-backend test --output json --gas 2000000 --amount ${coin.amount}${networks.MARS}'`));
    // should success, but later on channel balance will increase again
    console.log("transfer back msg: ", transferBackMsg);
    assert.deepEqual(transferBackMsg.code, 0);

    // query channel balance before
    let channelBalanceBefore = queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom));
    channelBalanceBefore = parseInt(channelBalanceBefore.native.amount);

    // we stop hermes docker so that the ibc transfer can timeout => refund
    execSync(`docker-compose stop hermes`);
    await sleep(25000); // sleep 25 sec to timeout transaction
    // start hermes docekr again after killing it
    execSync(`docker-compose up -d hermes`);
    spawnHermes(true); // spawn new hermes to create timeout ack
    await sleep(15000); // wait so hermes can create timeout ack tx
    console.log("after spawning hermes & relaying timeout ack");

    const channelBalanceAfter = parseInt(queryContractState(networks.MARS, testData.cwIcs20Address, parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })).data.balances.find(balance => balance.native.denom.includes(coin.denom)).native.amount);
    assert.deepEqual(channelBalanceAfter, channelBalanceBefore + 1); // should refund because out channel balance > in channel balance.
    const balanceAfter = queryNativeBalance(networks.MARS, mainMarsAddress, networks.MARS);
    assert.deepEqual(balanceBefore, balanceAfter); // should refund because out channel balance > in channel balance.
}

start();