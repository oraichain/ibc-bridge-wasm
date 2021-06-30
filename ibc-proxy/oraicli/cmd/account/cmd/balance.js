const run = async (yargs) => {
  const argv = await yargs.option('address', {
    describe: 'the receiver address',
    type: 'string'
  }).argv;
  const address = argv.address || argv.from_address;

  try {
    const data = await cosmos.get(`/cosmos/bank/v1beta1/balances/${address}`);
    data.address = address;
    console.log(data);
  } catch (ex) {
    console.log(ex);
  }
};

module.exports = run;
