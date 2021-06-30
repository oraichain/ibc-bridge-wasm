const run = (yargs) => {
  yargs
    .usage('usage: $0 ibc <command>')
    .command('transfer', 'transfer fungible token', require('./cmd/transfer'));
};

module.exports = run;
