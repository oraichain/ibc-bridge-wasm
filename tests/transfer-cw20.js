const fs = require('fs');
const path = require("path");

const { execSync } = require('child_process');
const assert = require('assert');

// We have this address's key, so it is used to create txs 
const mainMarsAddress = "mars15ez8l0c2qte2sa0a4xsdmaswy96vzj2fl2ephq";

async function start() {

    // should in form: {"cwIcs20Address":"mars1fventeva948ue0fzhp6xselr522rnqwger9wg7r0g9f4jemsqh6seanjnj","cw20Address":"mars18yn206ypuxay79gjqv6msvd9t2y49w4fz8q7fyenx5aggj0ua37q9sm4kk","channelId":"channel-5"}
    const testData = JSON.parse(fs.readFileSync(path.join(__dirname, 'test-data.json')));

    try {

        console.log("test ibc transfer to cw20 success");
        await testIbcTransferCw20SuccessShouldIncreaseCw20Balance(testData);

        console.log("test ibc transfer to cw20 no mapping should refund");
        await testIbcTransferCw20FailNoPairMappingShouldRefund(testData);

        console.log("test ibc transfer failed insufficient funds");
        await testIbcTransferCw20FailInsufficientFundsShouldRefund(testData);

        console.log("test ibc transfer native success should increase fund");
        await testIbcTransferNativeSuccessShouldIncreaseNativeBalance(testData);

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

async function sleep() {
    console.log("Sleeping to wait for IBC relayer to do its job ...");
    return await new Promise((resolve) => setTimeout(resolve, 20000)); // 20000ms = 20s = 4 blocks, should finish IBC transactions by now
}

async function testIbcTransferCw20SuccessShouldIncreaseCw20Balance(testData) {

    const coin = {
        amount: 1,
        denom: "uusd"
    }

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));

    assert.deepEqual(ibcTransferResult.code, 0);

    // query balance before receiving new tokens
    const balanceBefore = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cw20Address} ${parseDockerMessage({ "balance": { "address": mainMarsAddress } })} --output json'`))).data.balance);

    // query channel balance before
    let channelBalanceBefore = JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom));

    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    const balanceAfter = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cw20Address} ${parseDockerMessage({ "balance": { "address": mainMarsAddress } })} --output json'`))).data.balance);

    // channel balance after
    const channelBalanceAfter = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom)).native.amount);

    assert.deepEqual(balanceBefore + coin.amount, balanceAfter);
    assert.deepEqual(channelBalanceBefore + coin.amount, channelBalanceAfter);
}

async function testIbcTransferCw20FailNoPairMappingShouldRefund(testData) {

    const coin = {
        amount: 1,
        denom: "uatom"
    }

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));

    assert.deepEqual(ibcTransferResult.code, 0);

    // query channel balance before
    let channelBalanceBefore = JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom));

    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    // channel balance after
    let channelBalanceAfter = JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom));
    channelBalanceAfter = channelBalanceAfter ? parseInt(channelBalanceAfter.native.amount) : 0;

    assert.deepEqual(channelBalanceBefore, channelBalanceAfter);
}

async function testIbcTransferCw20FailInsufficientFundsShouldRefund(testData) {

    const coin = {
        amount: 1000,
        denom: "uusd"
    }

    // try sending ibc tokens, should succeed with code 0. uusd is mapped with a cw20
    const ibcTransferResult = JSON.parse(Buffer.from(execSync(`docker-compose exec -T earth ash -c 'oraid tx ibc-transfer transfer transfer ${testData.channelId} ${mainMarsAddress} ${coin.amount}${coin.denom} --from $USER --chain-id $CHAIN_ID -y --keyring-backend test -b block --output json'`)));

    assert.deepEqual(ibcTransferResult.code, 0);

    // query balance before receiving new tokens
    const balanceBefore = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cw20Address} ${parseDockerMessage({ "balance": { "address": mainMarsAddress } })} --output json'`))).data.balance);

    // query channel balance before
    let channelBalanceBefore = JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom));

    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    const balanceAfter = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cw20Address} ${parseDockerMessage({ "balance": { "address": mainMarsAddress } })} --output json'`))).data.balance);

    // channel balance after
    const channelBalanceAfter = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom)).native.amount);

    assert.deepEqual(balanceBefore, balanceAfter);
    assert.deepEqual(channelBalanceBefore, channelBalanceAfter);
}

async function testIbcTransferNativeSuccessShouldIncreaseNativeBalance(testData) {

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
    const balanceBefore = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query bank balances ${mainMarsAddress} --output json'`))).balances[0].amount);

    // query channel balance before
    let channelBalanceBefore = JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom));

    channelBalanceBefore = channelBalanceBefore ? parseInt(channelBalanceBefore.native.amount) : 0;

    // simple sleep to wait for ibc tokens to be tranferred before querying. An alternative is using ws
    await sleep();

    const balanceAfter = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query bank balances ${mainMarsAddress} --output json'`))).balances[0].amount);

    // channel balance after
    const channelBalanceAfter = parseInt(JSON.parse(Buffer.from(execSync(`docker-compose exec -T mars ash -c 'oraid query wasm contract-state smart ${testData.cwIcs20Address} ${parseDockerMessage({
        "channel": {
            "id"
                : testData.channelId
        }
    })} --output json'`))).data.balances.find(balance => balance.native.denom.includes(coin.denom)).native.amount);

    assert.deepEqual(balanceBefore + coin.amount, balanceAfter);
    assert.deepEqual(channelBalanceBefore + coin.amount, channelBalanceAfter);
}

start();