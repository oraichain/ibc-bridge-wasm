const run = (yargs) => {
  yargs
    .usage('usage: $0 account <command>')
    .command('create', 'create account', require('./cmd/create'))
    .command('balance', 'get account balance', require('./cmd/balance'));
};

module.exports = run;
