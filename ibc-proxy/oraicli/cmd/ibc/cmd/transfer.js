const run = async (yargs) => {
  const argv = await yargs
    .option('address', {
      describe: 'the receiver address',
      type: 'string'
    })
    .option('amount', {
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
    }).argv;

  const { denom, channel, port, from_address, address, amount, timeout } = argv;

  const msg = {
    source_channel: channel,
    source_port: port,
    sender: from_address,
    receiver: address,
    token: { denom, amount },
    timeout_timestamp: (Date.now() + timeout * 1000) * 10 ** 6
  };

  await submit('ibc.applications.transfer.v1.MsgTransfer', msg, argv);
};

module.exports = run;
