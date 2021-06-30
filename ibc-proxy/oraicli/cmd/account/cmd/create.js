const run = async (yargs) => {
  const argv = await yargs.option('length', {
    describe: 'the mnemonic length',
    type: 'number',
    default: 256
  }).argv;

  const mnemonic = cosmos.generateMnemonic(argv.length);
  const address = cosmos.getAddress(mnemonic);
  console.log({ mnemonic, address });
};

module.exports = run;
