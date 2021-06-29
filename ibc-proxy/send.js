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

const getIbcSendMsg = (
  from_address,

  { denom, channel, port, amount, address, timeout }
) => {
  const msgSend = new message.ibc.applications.transfer.v1.MsgTransfer({
    source_channel: channel,
    source_port: port,
    sender: from_address,
    receiver: address,
    token: { denom, amount },
    timeout_timestamp: (Date.now() + timeout * 1000) * 10 ** 6
  });

  return new message.google.protobuf.Any({
    type_url: '/ibc.applications.transfer.v1.MsgTransfer',
    value:
      message.ibc.applications.transfer.v1.MsgTransfer.encode(msgSend).finish()
  });
};

const getSendMsg = (from_address, { denom, amount, address }) => {
  const msgSend = new message.cosmos.bank.v1beta1.MsgSend({
    from_address,
    to_address: address,
    amount: [
      {
        denom,
        amount
      }
    ] // 100
  });

  return new message.google.protobuf.Any({
    type_url: '/cosmos.bank.v1beta1.MsgSend',
    value: message.cosmos.bank.v1beta1.MsgSend.encode(msgSend).finish()
  });
};

const run = async () => {
  let cosmos;
  const argv = await yargs(hideBin(process.argv))
    .middleware(async ({ network }) => {
      // global
      const chainId = network[0].toUpperCase() + network.substr(1);
      cosmos = new Cosmos(`http://lcd.${network}`, chainId);
      cosmos.setBech32MainPrefix(network);
      const mnemonic = await getLastLine(`accounts/${chainId}.txt`);
      return { mnemonic };
    })
    .alias('help', 'h')
    .alias('version', 'v')
    .option('address', {
      describe: 'the receiver address',
      type: 'string'
    })
    .option('network', {
      default: 'earth',
      type: 'string'
    })
    .option('timeout', {
      default: 60,
      type: 'number'
    })
    .option('port', {
      default: 'transfer',
      type: 'string'
    })
    .option('channel', {
      type: 'string'
    })
    .option('amount', {
      type: 'string'
    })
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
  argv.denom = cosmos.bech32MainPrefix;
  const childKey = cosmos.getChildKey(argv.mnemonic);
  const from_address = cosmos.getAddress(childKey);

  if (!argv.amount) {
    const checkAddr = argv.address || from_address;
    const data = await cosmos.get(`/cosmos/bank/v1beta1/balances/${checkAddr}`);
    console.log('balance of: ', checkAddr, data);
    return;
  }
  const msgSendAny = argv.channel
    ? getIbcSendMsg(from_address, argv)
    : getSendMsg(from_address, argv);

  const txBody = new message.cosmos.tx.v1beta1.TxBody({
    messages: [msgSendAny],
    memo: argv.memo
  });

  try {
    const response = await cosmos.submit(
      childKey,
      txBody,
      'BROADCAST_MODE_BLOCK',
      isNaN(argv.fees) ? 0 : parseInt(argv.fees)
    );
    console.log(response);
  } catch (ex) {
    console.log(ex);
  }
};

run();
