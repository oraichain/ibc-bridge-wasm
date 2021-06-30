const run = async (yargs) => {
  const argv = await yargs
    .option('address', {
      describe: 'the receiver address',
      type: 'string'
    })
    .option('amount', {
      type: 'string'
    }).argv;

  const { denom, amount, from_address, address } = argv;

  const msg = {
    from_address,
    to_address: address,
    amount: [
      {
        denom,
        amount
      }
    ]
  };

  await submit('cosmos.bank.v1beta1.MsgSend', msg, argv);
};

module.exports = run;
