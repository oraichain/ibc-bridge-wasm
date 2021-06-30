const fs = require('fs');
const readline = require('readline');

const passAccount = (fileName, network) => {
  return new Promise((resolve, reject) => {
    const rl = readline.createInterface(fs.createReadStream(fileName));
    let lastLine = '';
    const account = { network };
    rl.on('line', (line) => {
      if (line.length > 0) {
        const match = line.match(/(\w+)\s*:\s*([\w\d]+)/);
        if (match) {
          account[match[1]] = match[2];
        } else {
          lastLine = line;
        }
      }
    });

    rl.on('error', reject);

    rl.on('close', () => {
      account.mnemonic = lastLine;
      resolve(account);
    });
  });
};
const getAccounts = async (path) => {
  const dirs = fs.readdirSync(path);
  const accounts = await Promise.all(
    dirs.map((dir) => passAccount(`${path}/${dir}`, dir.replace('.txt', '')))
  );
  return accounts;
};

module.exports = getAccounts;
