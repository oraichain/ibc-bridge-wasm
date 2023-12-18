#![cfg(test)]

use crate::contract::instantiate;
use crate::ibc::{ibc_channel_connect, ibc_channel_open, ICS20_ORDERING, ICS20_VERSION};
use crate::state::ChannelInfo;

use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};

use cosmwasm_std::{
    coins, Addr, Api, Binary, CanonicalAddr, DepsMut, IbcChannel, IbcChannelConnectMsg,
    IbcChannelOpenMsg, IbcEndpoint, OwnedDeps,
};
use cosmwasm_testing_util::mock::MockContract;
use cosmwasm_vm::testing::MockInstanceOptions;

use crate::msg::{AllowMsg, InitMsg};

pub const DEFAULT_TIMEOUT: u64 = 3600; // 1 hour,
pub const CONTRACT_PORT: &str = "wasm.cosmos2contract"; // wasm.MOCK_CONTRACT_ADDR
pub const REMOTE_PORT: &str = "transfer";
pub const CONNECTION_ID: &str = "connection-2";

const WASM_BYTES: &[u8] = include_bytes!("../artifacts/cw-ics20-latest.wasm");
const SENDER: &str = "orai1gkr56hlnx9vc7vncln2dkd896zfsqjn300kfq0";
const CONTRACT: &str = "orai19p43y0tqnr5qlhfwnxft2u5unph5yn60y7tuvu";

pub fn mock_channel(channel_id: &str) -> IbcChannel {
    IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.into(),
            channel_id: channel_id.into(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.into(),
            channel_id: format!("{}5", channel_id),
        },
        ICS20_ORDERING,
        ICS20_VERSION,
        CONNECTION_ID,
    )
}

pub fn mock_channel_info(channel_id: &str) -> ChannelInfo {
    ChannelInfo {
        id: channel_id.to_string(),
        counterparty_endpoint: IbcEndpoint {
            port_id: REMOTE_PORT.into(),
            channel_id: format!("{}5", channel_id),
        },
        connection_id: CONNECTION_ID.into(),
    }
}

// we simulate instantiate and ack here
pub fn add_channel(mut deps: DepsMut, channel_id: &str) {
    let channel = mock_channel(channel_id);
    let open_msg = IbcChannelOpenMsg::new_init(channel.clone());
    ibc_channel_open(deps.branch(), mock_env(), open_msg).unwrap();
    let connect_msg = IbcChannelConnectMsg::new_ack(channel, ICS20_VERSION);
    ibc_channel_connect(deps.branch(), mock_env(), connect_msg).unwrap();
}

pub fn setup(
    channels: &[&str],
    allow: &[(&str, u64)],
) -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    let mut deps = mock_dependencies();

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
        gov_contract: "gov".to_string(),
        allowlist,
        swap_router_contract: "router".to_string(),
        converter_contract: "converter".to_string(),
    };
    let info = mock_info(&String::from("anyone"), &[]);
    let res = instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();
    assert_eq!(0, res.messages.len());

    for channel in channels {
        add_channel(deps.as_mut(), channel);
    }
    deps
}

use anybuf::{Anybuf, Bufany};

#[test]
pub fn test_memo() {
    let contract_instance = MockContract::new(
        WASM_BYTES,
        Addr::unchecked(CONTRACT),
        MockInstanceOptions {
            balances: &[(SENDER, &coins(100_000_000_000, "orai"))],
            gas_limit: 40_000_000_000_000_000,
            ..MockInstanceOptions::default()
        },
    );

    let memo = Binary::from(
        Anybuf::new()
            .append_bytes(
                1,
                contract_instance
                    .api()
                    .addr_canonicalize("orai1ntdmh848kktumfw5tx8l2semwkxa5s7e5rs03x")
                    .unwrap()
                    .as_slice(),
            ) // receiver on Oraichain
            .append_string(2, "orai1ntdmh848kktumfw5tx8l2semwkxa5s7e5rs03x") // destination receiver
            .append_string(3, "channel-19") // destination channel
            .append_string(
                4, "ibc/", //destination denom
            )
            .as_bytes(),
    )
    .to_base64();

    println!("memo {}", memo);

    let data = Binary::from_base64(&memo).unwrap();

    let deserialized = Bufany::deserialize(&data).unwrap();

    let receiver = contract_instance
        .api()
        .addr_humanize(&CanonicalAddr::from(deserialized.bytes(1).unwrap()))
        .unwrap();
    let destination_receiver = deserialized.string(2).unwrap();
    let destination_channel = deserialized.string(3).unwrap();
    let destination_denom = deserialized.string(4).unwrap();

    println!(
        "{}-{}-{}-{}",
        receiver, destination_receiver, destination_channel, destination_denom
    );
}
