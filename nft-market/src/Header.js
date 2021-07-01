import { useEffect, useRef, useState } from 'react';

window.seedData = async (mnemonic, startId = 4) => {
  const { photos } = require('./data.json');

  const childKey = window.wasm.cosmos.getChildKey(mnemonic);

  for (let { src, width, height, title, price } of photos) {
    const realPrice =
      window.BigInt(price.match(/\d+/)[0]) * window.BigInt(1000000);
    const msg = {
      name: title,
      description: `${width}x${height}`,
      image: src,
      tokenId: (startId++).toString(),
      price: realPrice.toString(10)
    };
    const ret = await window.wasm.sellNft(msg, childKey);
    console.log(ret);
  }
};

const parseDenom = (denom) => {
  switch (denom) {
    case 'ibc/1D87F7F49C0E994F34935219BEB178D8D1E11DB9B94208DD0004ACA7C4E1D767':
      return 'earth';
    case 'ibc/05444EFC83A16B5CBA7DE8AFD12EE3DDA503AFE4FDDF0222925B89EF02D10041':
      return 'mars';
    default:
      return denom;
  }
};

const Header = () => {
  const [state, setState] = useState({
    showAccount: false,
    currentAccount: null,
    accounts: []
  });
  const toggleShowAccount = () => {
    setState((props) => ({
      ...props,
      showAccount: !props.showAccount
    }));
  };
  const selectAccount = (ind) => {
    setState((props) => ({ ...props, currentAccount: props.accounts[ind] }));
  };
  useEffect(() => {
    const getAccounts = async () => {
      const accounts = await fetch(`https://ibc.orai.io/accounts`).then((res) =>
        res.json()
      );

      const mapAccount = new Map(
        accounts.map((account) => [account.network, account])
      );

      for (let account of accounts) {
        const getBalance = async (address) => {
          const url = `https://lcd.${account.network.toLowerCase()}.orai.io/cosmos/bank/v1beta1/balances/${address}`;

          const { balances } = await fetch(url).then((res) => res.json());

          return balances
            .map((balance) => `${balance.amount} ${parseDenom(balance.denom)}`)
            .join(', ');
        };

        account.balance = await getBalance(account.address);

        if (account.network === 'mars') {
          const earthAccount = mapAccount.get('earth');
          earthAccount.marsAddress = window.wasm.cosmos.getAddress(
            earthAccount.mnemonic
          );
          earthAccount.marsBalance = await getBalance(earthAccount.marsAddress);
          account.earthAccount = earthAccount;
        }
      }

      setState((props) => ({ ...props, accounts }));
    };
    getAccounts();
  }, []);

  const formRef = useRef();
  const sellNft = async () => {
    const formData = new FormData(formRef.current);
    const msg = {
      name: formData.get('name'),
      description: formData.get('description'),
      image: formData.get('image'),
      tokenId: formData.get('tokenId'),
      price: formData.get('price')
    };

    const childKey = window.wasm.cosmos.getChildKey(
      state.currentAccount.mnemonic
    );

    try {
      const ret = await window.wasm.sellNft(msg, childKey);
      console.log(ret);
    } catch (ex) {
      alert(ex.message);
    }
  };

  return (
    <>
      <header>
        <h2>NFT Marketplace on IBC</h2>
        <button onClick={toggleShowAccount}>
          {state.currentAccount
            ? `${state.currentAccount.name} (${state.currentAccount.network}) ${state.currentAccount.balance}`
            : 'Accounts'}
        </button>
      </header>
      {state.showAccount && (
        <table>
          <thead>
            <tr>
              <th>Account</th>
              <th>Address</th>
              <th>Network</th>
            </tr>
          </thead>
          <tbody>
            {state.accounts?.map(
              ({ name, address, network, balance, earthAccount }, ind) => (
                <tr key={address} onClick={() => selectAccount(ind)}>
                  <td>{name}</td>
                  <td>
                    {address} {balance}
                    {earthAccount && (
                      <div>
                        {earthAccount.marsAddress} {earthAccount.marsBalance}
                      </div>
                    )}
                  </td>
                  <td>{network}</td>
                </tr>
              )
            )}
          </tbody>
        </table>
      )}
      {state.currentAccount && (
        <div style={{ padding: 20 }}>
          <form ref={formRef}>
            <label htmlFor="name">NFT Name</label>
            <input type="text" name="name" placeholder="NFT name.." />

            <label htmlFor="description">Description</label>
            <input
              type="text"
              name="description"
              placeholder="NFT descripiton.."
            />

            <label htmlFor="image">Image</label>
            <input type="text" name="image" placeholder="NFT URI.." />

            <label htmlFor="tokenId">Token ID</label>
            <input type="text" name="tokenId" placeholder="Token ID.." />

            <label htmlFor="price">Price</label>
            <input type="text" name="price" placeholder="Price.." />

            <input type="button" value="Submit" onClick={sellNft} />
          </form>
        </div>
      )}
    </>
  );
};

export default Header;
