const run = async (yargs) => {
  const argv = await yargs.option('address', {
    describe: 'the smart contract address',
    type: 'string'
  }).argv;

  const { address } = argv;
  const data = await cosmos.get(
    `/wasm/v1beta1/contract/${address}/smart/${Buffer.from(argv.input).toString(
      'base64'
    )}`
  );
  console.log(data.data);
};

module.exports = run;
