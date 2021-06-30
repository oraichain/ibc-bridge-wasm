const run = async (yargs) => {
  const argv = await yargs
    .option('address', {
      describe: 'the smart contract address',
      type: 'string'
    })
    .option('amount', {
      type: 'string'
    }).argv;

  const { address, from_address, amount, denom } = argv;

  const input = Buffer.from(argv.input);
  const sent_funds = amount ? [{ denom, amount }] : null;
  const msg = {
    contract: address,
    msg: input,
    sender: from_address,
    sent_funds
  };
  await submit('cosmwasm.wasm.v1beta1.MsgExecuteContract', msg, argv);
};

module.exports = run;
