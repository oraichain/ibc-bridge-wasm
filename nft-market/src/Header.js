import { useEffect, useState } from 'react';

const port = process.env.REACT_APP_SERVER_PORT || 8080;
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
      const accounts = await fetch(`http://localhost:${port}/accounts`).then(
        (res) => res.json()
      );

      for (let account of accounts) {
        const url = `http://lcd.${account.network.toLowerCase()}:${port}/cosmos/bank/v1beta1/balances/${
          account.address
        }`;

        const { balances } = await fetch(url).then((res) => res.json());
        const balance = balances[0];
        account.balance = `${balance.amount} ${balance.denom}`;
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
