use anybuf::Anybuf;
use cosmwasm_std::{
    coin, coins, testing::mock_env, Addr, Api, BankMsg, Binary, CosmosMsg, IbcMsg, StdError, SubMsg,
};
use cosmwasm_testing_util::mock::{MockApi, MockContract};
use cosmwasm_vm::testing::MockInstanceOptions;
use cw20_ics20_msg::{ibc_hooks::HookMethods, msg::UpdatePairMsg, state::Ratio};
use oraiswap::asset::AssetInfo;

use crate::{
    msg::{AllowMsg, ExecuteMsg, InitMsg},
    state::TOKEN_FEE,
    testing::test_helpers::{DEFAULT_TIMEOUT, WASM_BYTES},
};

const SENDER: &str = "orai1gkr56hlnx9vc7vncln2dkd896zfsqjn300kfq0";
const CONTRACT: &str = "orai19p43y0tqnr5qlhfwnxft2u5unph5yn60y7tuvu";

fn build_ibc_hooks_universal_swap_args(
    receiver: String,
    destination_receiver: String,
    destination_channel: String,
    destination_denom: String,
) -> Binary {
    let mock_api = MockApi::default();
    let args = Binary::from(
        Anybuf::new()
            .append_bytes(1, mock_api.addr_canonicalize(&receiver).unwrap().as_slice())
            .append_string(2, &destination_receiver)
            .append_string(3, &destination_channel)
            .append_string(4, &destination_denom)
            .as_bytes(),
    )
    .to_base64();

    return Binary::from_base64(&args).unwrap();
}

#[test]
fn test_ibc_hooks_receive() {
    let mut contract_instance = MockContract::new(
        WASM_BYTES,
        Addr::unchecked(CONTRACT),
        MockInstanceOptions {
            balances: &[(SENDER, &coins(100_000_000_000, "orai"))],
            gas_limit: 40_000_000_000_000_000,
            ..MockInstanceOptions::default()
        },
    );

    let cw20_addr = "orai1lus0f0rhx8s03gdllx2n6vhkmf0536dv57wfge";
    let send_channel = "channel-9";
    let denom = "uatom0x";
    let asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked(cw20_addr),
    };
    let gas_limit = 1234567;

    let allow = &[(cw20_addr, gas_limit)];

    let allowlist = allow
        .iter()
        .map(|(contract, gas)| AllowMsg {
            contract: contract.to_string(),
            gas_limit: Some(*gas),
        })
        .collect();

    // instantiate an empty contract
    let instantiate_msg = InitMsg {
        default_gas_limit: None,
        default_timeout: DEFAULT_TIMEOUT,
        gov_contract: SENDER.to_string(),
        allowlist,
        swap_router_contract: "router".to_string(),
        converter_contract: "converter".to_string(),
    };

    contract_instance
        .instantiate(instantiate_msg, SENDER, &[])
        .unwrap();

    contract_instance
        .with_storage(|storage| {
            TOKEN_FEE
                .save(
                    storage,
                    denom,
                    &Ratio {
                        nominator: 1,
                        denominator: 10,
                    },
                )
                .unwrap();
            Ok(())
        })
        .unwrap();

    let pair = UpdatePairMsg {
        local_channel_id: send_channel.to_string(),
        denom: denom.to_string(),
        local_asset_info: asset_info.clone(),
        remote_decimals: 18u8,
        local_asset_info_decimals: 18u8,
        is_mint_burn: None,
    };

    contract_instance
        .execute(ExecuteMsg::UpdateMappingPair(pair), SENDER, &[])
        .unwrap();

    // case 1: Destination on Oraichain
    let args = build_ibc_hooks_universal_swap_args(
        SENDER.to_string(),
        SENDER.to_string(),
        "".to_string(),
        "".to_string(),
    );

    let res = contract_instance
        .execute(
            ExecuteMsg::IbcHooksReceive {
                func: HookMethods::UniversalSwap,
                args: args,
            },
            SENDER,
            &vec![coin(100_000_000_000, "ibc/orai")],
        )
        .unwrap();
    assert_eq!(
        res.0.messages,
        vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "orai1gkr56hlnx9vc7vncln2dkd896zfsqjn300kfq0".to_string(),
            amount: vec![coin(100_000_000_000, "ibc/orai")]
        }))]
    );

    assert_eq!(
        res.0.attributes,
        vec![
            ("action", "receive_ibc_hooks"),
            ("receiver", "orai1gkr56hlnx9vc7vncln2dkd896zfsqjn300kfq0"),
            ("denom", "ibc/orai"),
            ("amount", "100000000000"),
            ("token_fee", "0"),
            ("relayer_fee", "0")
        ]
    );

    // case 2: destination is others chain

    // failed because destination denom is empty
    let args = build_ibc_hooks_universal_swap_args(
        SENDER.to_string(),
        "cosmos2cosmos".to_string(),
        "channel-15".to_string(),
        "".to_string(),
    );
    let res = contract_instance
        .execute(
            ExecuteMsg::IbcHooksReceive {
                func: HookMethods::UniversalSwap,
                args: args,
            },
            SENDER,
            &vec![coin(100_000_000_000, "ibc/orai")],
        )
        .unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("Require destination denom & channel in memo".to_string())
            .to_string()
    );

    // will be successful
    let args = build_ibc_hooks_universal_swap_args(
        SENDER.to_string(),
        "cosmos1gkr56hlnx9vc7vncln2dkd896zfsqjn3uuq2pu".to_string(),
        "channel-15".to_string(),
        "uatom".to_string(),
    );
    let res = contract_instance
        .execute(
            ExecuteMsg::IbcHooksReceive {
                func: HookMethods::UniversalSwap,
                args: args,
            },
            SENDER,
            &vec![coin(
                100_000_000_000,
                "ibc/A2E2EEC9057A4A1C2C0A6A4C78B0239118DF5F278830F50B4A6BDD7A66506B78",
            )],
        )
        .unwrap();

    assert_eq!(
        res.0.messages,
        vec![SubMsg::new(CosmosMsg::Ibc(IbcMsg::Transfer {
            channel_id: "channel-15".to_string(),
            to_address: "cosmos1gkr56hlnx9vc7vncln2dkd896zfsqjn3uuq2pu".to_string(),
            amount: coin(
                100_000_000_000,
                "ibc/A2E2EEC9057A4A1C2C0A6A4C78B0239118DF5F278830F50B4A6BDD7A66506B78"
            ),
            timeout: mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT).into()
        }))]
    );
    assert_eq!(
        res.0.attributes,
        vec![
            ("action", "receive_ibc_hooks"),
            ("receiver", "cosmos1gkr56hlnx9vc7vncln2dkd896zfsqjn3uuq2pu"),
            (
                "denom",
                "ibc/A2E2EEC9057A4A1C2C0A6A4C78B0239118DF5F278830F50B4A6BDD7A66506B78"
            ),
            ("amount", "100000000000"),
            ("token_fee", "0"),
            ("relayer_fee", "0")
        ]
    );
}
