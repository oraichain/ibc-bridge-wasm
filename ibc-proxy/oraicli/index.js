const yargs = require('yargs/yargs');
const { hideBin } = require('yargs/helpers');
const fs = require('fs');
const readline = require('readline');
const Cosmos = require('@oraichain/cosmosjs').default;
const message = Cosmos.message;

const getLastLine = (fileName) => {
  return new Promise((resolve, reject) => {
    const rl = readline.createInterface(fs.createReadStream(fileName));
    let lastLine = '';
    rl.on('line', (line) => {
      if (line.length > 0) {
        lastLine = line;
      }
    });

    rl.on('error', reject);

    rl.on('close', () => {
      resolve(lastLine);
    });
  });
};

global.submit = async (type, obj, { childKey, memo, fees, gas }) => {
  const paths = type.split('.');
  let childMessage = message;
  for (let p of paths) childMessage = childMessage[p];

  const msgSend = new childMessage(obj);

  const msgSendAny = new message.google.protobuf.Any({
    type_url: `/${type}`,
    value: childMessage.encode(msgSend).finish()
  });

  const txBody = new message.cosmos.tx.v1beta1.TxBody({
    messages: [msgSendAny],
    memo
  });

  try {
    const response = await cosmos.submit(
      childKey,
      txBody,
      'BROADCAST_MODE_BLOCK',
      isNaN(fees) ? 0 : parseInt(fees),
      gas
    );
    // log response then return
    console.log(response);
    return response;
  } catch (ex) {
    console.log(ex);
  }
};

const run = () => {
  yargs(hideBin(process.argv))
    .middleware(async ({ network }) => {
      // global
      const chainId = network[0].toUpperCase() + network.substr(1);
      global.cosmos = new Cosmos(`http://lcd.${network}`, chainId);
      cosmos.setBech32MainPrefix(network);
      const mnemonic =
        process.env.MNEMONIC || (await getLastLine(`accounts/${chainId}.txt`));
      const childKey = cosmos.getChildKey(mnemonic);
      const from_address = cosmos.getAddress(childKey);
      return { mnemonic, denom: network, childKey, from_address };
    })
    .alias('help', 'h')
    .alias('version', 'v')

    .option('network', {
      default: 'earth',
      type: 'string'
    })
    .command('send [address]', 'send orai token', require('./cmd/send'))
    .command('account', 'account commands', require('./cmd/account'))
    .command('wasm', 'wasm commands', require('./cmd/wasm'))
    .command('ibc', 'ibc commands', require('./cmd/ibc'))

    .option('memo', {
      default: '',
      type: 'string'
    })
    .option('gas', {
      type: 'number',
      default: 2000000
    })
    .option('fees', {
      describe: 'the transaction fees',
      type: 'string'
    }).argv;
};

run();
