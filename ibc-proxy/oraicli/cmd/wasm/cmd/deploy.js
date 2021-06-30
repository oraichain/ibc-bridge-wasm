const fs = require('fs');

const run = async (yargs) => {
  const argv = await yargs
    .option('file', {
      describe: 'the smart contract file',
      type: 'string'
    })
    .option('label', {
      describe: 'the label of smart contract',
      type: 'string'
    })
    .option('amount', {
      type: 'string'
    }).argv;

  const { file, from_address, label, amount, denom } = argv;
  const wasm_byte_code = fs.readFileSync(file);

  const msg1 = {
    wasm_byte_code,
    sender: from_address
  };

  const res1 = await submit('cosmwasm.wasm.v1beta1.MsgStoreCode', msg1, argv);

  if (res1.tx_response.code !== 0) {
    return;
  }

  // next instantiate code
  const codeId = res1.tx_response.logs[0].events[0].attributes.find(
    (attr) => attr.key === 'code_id'
  ).value;
  const input = Buffer.from(argv.input);
  const sent_funds = amount ? [{ denom, amount }] : null;
  const msg2 = {
    code_id: codeId,
    init_msg: input,
    label,
    sender: from_address,
    sent_funds
  };

  const res2 = await submit(
    'cosmwasm.wasm.v1beta1.MsgInstantiateContract',
    msg2,
    argv
  );

  if (res2.tx_response.code !== 0) {
    return;
  }

  let address = JSON.parse(res2.tx_response.raw_log)[0].events[1].attributes[0]
    .value;

  console.log('contract address: ', address);
};

module.exports = run;
