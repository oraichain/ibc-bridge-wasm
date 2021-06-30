const run = (yargs) => {
  yargs
    .usage('usage: $0 wasm <command> [options]')
    .command('query', 'query a smart contract', require('./cmd/query'))
    .command('execute', 'execute a smart contract', require('./cmd/execute'))
    .command('deploy', 'deploy a smart contract', require('./cmd/deploy'))
    .option('input', {
      describe: 'the input to initilize smart contract',
      default: '{}',
      type: 'string'
    });
};

module.exports = run;
