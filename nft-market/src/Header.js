import { useEffect, useState } from 'react';

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

const port = process.env.REACT_APP_SERVER_PORT
  ? ':' + process.env.REACT_APP_SERVER_PORT
  : '';
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
      const accounts = await fetch(`http://localhost${port}/accounts`).then(
        (res) => res.json()
      );

      for (let account of accounts) {
        const url = `http://lcd.${account.network.toLowerCase()}${port}/cosmos/bank/v1beta1/balances/${
          account.address
        }`;

        const { balances } = await fetch(url).then((res) => res.json());

        account.balance = balances
          .map((balance) => `${balance.amount} ${parseDenom(balance.denom)}`)
          .join(', ');
      }

      setState((props) => ({ ...props, accounts }));
    };
    getAccounts();
  }, []);

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
            {state.accounts?.map(({ name, address, network, balance }, ind) => (
              <tr key={address} onClick={() => selectAccount(ind)}>
                <td>{name}</td>
                <td>
                  {address} {balance}
                </td>
                <td>{network}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </>
  );
};

export default Header;
