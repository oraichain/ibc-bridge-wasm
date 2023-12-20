use std::ops::Sub;

use cosmwasm_std::{
    coin, Addr, BankMsg, CosmosMsg, Decimal, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcTimeout,
    StdError,
};
use cosmwasm_testing_util::mock::MockContract;
use cosmwasm_vm::testing::MockInstanceOptions;
use cw20_ics20_msg::receiver::DestinationInfo;
use cw_controllers::AdminError;
use oraiswap::asset::AssetInfo;
use oraiswap::router::{RouterController, SwapOperation};

use crate::ibc::{
    build_ibc_msg, build_swap_msgs, convert_remote_denom_to_evm_prefix, deduct_fee,
    deduct_relayer_fee, deduct_token_fee, get_token_price, handle_packet_refund,
    ibc_packet_receive, parse_ibc_channel_without_sanity_checks,
    parse_ibc_denom_without_sanity_checks, parse_ibc_info_without_sanity_checks,
    parse_voucher_denom, process_ibc_msg, Ics20Ack, Ics20Packet, FOLLOW_UP_IBC_SEND_FAILURE_ID,
    IBC_TRANSFER_NATIVE_ERROR_ID, ICS20_VERSION, NATIVE_RECEIVE_ID, REFUND_FAILURE_ID,
    SWAP_OPS_FAILURE_ID,
};
use crate::ibc::{build_swap_operations, get_follow_up_msgs};
use crate::test_helpers::*;
use cosmwasm_std::{
    from_binary, to_binary, IbcEndpoint, IbcMsg, IbcPacket, IbcPacketReceiveMsg, SubMsg, Timestamp,
    Uint128, WasmMsg,
};

use crate::error::ContractError;
use crate::state::{
    get_key_ics20_ibc_denom, increase_channel_balance, reduce_channel_balance, ChannelState,
    MappingMetadata, Ratio, RelayerFee, TokenFee, CHANNEL_REVERSE_STATE, RELAYER_FEE, REPLY_ARGS,
    TOKEN_FEE,
};
use cw20::{Cw20Coin, Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw20_ics20_msg::amount::{convert_local_to_remote, Amount};

use crate::contract::{
    execute, handle_override_channel_balance, query, query_channel, query_channel_with_key,
};
use crate::msg::{
    AllowMsg, ChannelResponse, ConfigResponse, DeletePairMsg, ExecuteMsg, InitMsg,
    ListChannelsResponse, ListMappingResponse, PairQuery, QueryMsg, TransferBackMsg, UpdatePairMsg,
};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{coins, to_vec};

const WASM_BYTES: &[u8] = include_bytes!("../artifacts/cw-ics20-latest.wasm");
const SENDER: &str = "orai1gkr56hlnx9vc7vncln2dkd896zfsqjn300kfq0";
const CONTRACT: &str = "orai19p43y0tqnr5qlhfwnxft2u5unph5yn60y7tuvu";

#[test]
fn check_ack_json() {
    let success = Ics20Ack::Result(b"1".into());
    let fail = Ics20Ack::Error("bad coin".into());

    let success_json = String::from_utf8(to_vec(&success).unwrap()).unwrap();
    assert_eq!(r#"{"result":"MQ=="}"#, success_json.as_str());

    let fail_json = String::from_utf8(to_vec(&fail).unwrap()).unwrap();
    assert_eq!(r#"{"error":"bad coin"}"#, fail_json.as_str());
}

#[test]
fn test_sub_negative() {
    assert_eq!(
        Uint128::from(10u128)
            .checked_sub(Uint128::from(11u128))
            .unwrap_or_default(),
        Uint128::from(0u128)
    )
}

#[test]
fn check_packet_json() {
    let packet = Ics20Packet::new(
        Uint128::new(12345),
        "ucosm",
        "cosmos1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n",
        "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc",
        None,
    );
    // Example message generated from the SDK
    let expected = r#"{"amount":"12345","denom":"ucosm","receiver":"wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc","sender":"cosmos1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n","memo":null}"#;

    let encdoded = String::from_utf8(to_vec(&packet).unwrap()).unwrap();
    assert_eq!(expected, encdoded.as_str());
}

// #[test]
// fn check_gas_limit_handles_all_cases() {
//     let send_channel = "channel-9";
//     let allowed = "foobar";
//     let allowed_gas = 777666;
//     let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);

//     // allow list will get proper gas
//     let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, allowed)).unwrap();
//     assert_eq!(limit, Some(allowed_gas));

//     // non-allow list will error
//     let random = "tokenz";
//     check_gas_limit(deps.as_ref(), &Amount::cw20(500, random)).unwrap_err();

//     // add default_gas_limit
//     let def_limit = 54321;
//     migrate(
//         deps.as_mut(),
//         mock_env(),
//         MigrateMsg {
//             // default_gas_limit: Some(def_limit),
//             // token_fee_receiver: "receiver".to_string(),
//             // relayer_fee_receiver: "relayer_fee_receiver".to_string(),
//             // default_timeout: 100u64,
//             // fee_denom: "orai".to_string(),
//             // swap_router_contract: "foobar".to_string(),
//         },
//     )
//     .unwrap();

//     // allow list still gets proper gas
//     let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, allowed)).unwrap();
//     assert_eq!(limit, Some(allowed_gas));

//     // non-allow list will now get default
//     let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, random)).unwrap();
//     assert_eq!(limit, Some(def_limit));
// }

// test remote chain send native token to local chain
fn mock_receive_packet_remote_to_local(
    my_channel: &str,
    amount: u128,
    denom: &str,
    receiver: &str,
    sender: Option<&str>,
) -> IbcPacket {
    let data = Ics20Packet {
        // this is returning a foreign native token, thus denom is <denom>, eg: uatom
        denom: denom.to_string(),
        amount: amount.into(),
        sender: if sender.is_none() {
            "remote-sender".to_string()
        } else {
            sender.unwrap().to_string()
        },
        receiver: receiver.to_string(),
        memo: None,
    };
    IbcPacket::new(
        to_binary(&data).unwrap(),
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: "channel-1234".to_string(),
        },
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: my_channel.to_string(),
        },
        3,
        Timestamp::from_seconds(1665321069).into(),
    )
}

#[test]
fn test_parse_voucher_denom_invalid_length() {
    let voucher_denom = "foobar/foobar";
    let ibc_endpoint = IbcEndpoint {
        port_id: "hello".to_string(),
        channel_id: "world".to_string(),
    };
    // native denom case
    assert_eq!(
        parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap_err(),
        ContractError::NoForeignTokens {}
    );
}

#[test]
fn test_parse_voucher_denom_invalid_port() {
    let voucher_denom = "foobar/abc/xyz";
    let ibc_endpoint = IbcEndpoint {
        port_id: "hello".to_string(),
        channel_id: "world".to_string(),
    };
    // native denom case
    assert_eq!(
        parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap_err(),
        ContractError::FromOtherPort {
            port: "foobar".to_string()
        }
    );
}

#[test]
fn test_parse_voucher_denom_invalid_channel() {
    let voucher_denom = "hello/abc/xyz";
    let ibc_endpoint = IbcEndpoint {
        port_id: "hello".to_string(),
        channel_id: "world".to_string(),
    };
    // native denom case
    assert_eq!(
        parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap_err(),
        ContractError::FromOtherChannel {
            channel: "abc".to_string()
        }
    );
}

#[test]
fn test_parse_voucher_denom_native_denom_valid() {
    let voucher_denom = "foobar";
    let ibc_endpoint = IbcEndpoint {
        port_id: "hello".to_string(),
        channel_id: "world".to_string(),
    };
    assert_eq!(
        parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap(),
        ("foobar", true)
    );
}

/////////////////////////////// Test cases for native denom transfer from remote chain to local chain

#[test]
fn send_native_from_remote_mapping_not_found() {
    let relayer = Addr::unchecked("relayer");
    let send_channel = "channel-9";
    let cw20_addr = "token-addr";
    let custom_addr = "custom-addr";
    let cw20_denom = "cw20:token-addr";
    let gas_limit = 1234567;
    let mut deps = setup(
        &["channel-1", "channel-7", send_channel],
        &[(cw20_addr, gas_limit)],
    );

    // prepare some mock packets
    let recv_packet =
        mock_receive_packet_remote_to_local(send_channel, 876543210, cw20_denom, custom_addr, None);

    // we can receive this denom, channel balance should increase
    let msg = IbcPacketReceiveMsg::new(recv_packet.clone(), relayer);
    let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
    // assert_eq!(res, StdError)
    assert_eq!(
        res.attributes
            .into_iter()
            .find(|attr| attr.key.eq("error"))
            .unwrap()
            .value,
        "You can only send native tokens that has a map to the corresponding asset info"
    );
}

#[test]
fn proper_checks_on_execute_native_transfer_back_to_remote() {
    // arrange
    let relayer = Addr::unchecked("relayer");
    let remote_channel = "channel-5";
    let remote_address = "cosmos1603j3e4juddh7cuhfquxspl0p0nsun046us7n0";
    let custom_addr = "custom-addr";
    let original_sender = "original_sender";
    let denom = "uatom0x";
    let amount = 1234567u128;
    let token_addr = Addr::unchecked("token-addr".to_string());
    let asset_info = AssetInfo::Token {
        contract_addr: token_addr.clone(),
    };
    let cw20_raw_denom = token_addr.as_str();
    let local_channel = "channel-1234";
    let ibc_denom = get_key_ics20_ibc_denom("wasm.cosmos2contract", local_channel, denom);
    let ratio = Ratio {
        nominator: 1,
        denominator: 10,
    };
    let fee_amount =
        Uint128::from(amount) * Decimal::from_ratio(ratio.nominator, ratio.denominator);
    let mut deps = setup(&[remote_channel, local_channel], &[]);
    TOKEN_FEE
        .save(deps.as_mut().storage, denom, &ratio)
        .unwrap();

    let pair = UpdatePairMsg {
        local_channel_id: local_channel.to_string(),
        denom: denom.to_string(),
        local_asset_info: asset_info.clone(),
        remote_decimals: 18u8,
        local_asset_info_decimals: 18u8,
    };

    let _ = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("gov", &[]),
        ExecuteMsg::UpdateMappingPair(pair),
    )
    .unwrap();

    // execute
    let mut transfer = TransferBackMsg {
        local_channel_id: local_channel.to_string(),
        remote_address: remote_address.to_string(),
        remote_denom: denom.to_string(),
        timeout: Some(DEFAULT_TIMEOUT),
        memo: None,
    };

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: original_sender.to_string(),
        amount: Uint128::from(amount),
        msg: to_binary(&transfer).unwrap(),
    });

    // insufficient funds case because we need to receive from remote chain first
    let info = mock_info(cw20_raw_denom, &[]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());
    assert_eq!(
        res.unwrap_err(),
        ContractError::NoSuchChannelState {
            id: local_channel.to_string(),
            denom: get_key_ics20_ibc_denom("wasm.cosmos2contract", local_channel, denom)
        }
    );

    // prepare some mock packets
    let recv_packet =
        mock_receive_packet(remote_channel, local_channel, amount, denom, custom_addr);

    // receive some tokens. Assume that the function works perfectly because the test case is elsewhere
    let ibc_msg = IbcPacketReceiveMsg::new(recv_packet.clone(), relayer);
    ibc_packet_receive(deps.as_mut(), mock_env(), ibc_msg).unwrap();
    // need to trigger increase channel balance because it is executed through submsg
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(mock_env().contract.address.as_str(), &[]),
        ExecuteMsg::IncreaseChannelBalanceIbcReceive {
            dest_channel_id: local_channel.to_string(),
            ibc_denom: ibc_denom.clone(),
            amount: Uint128::from(amount),
            local_receiver: custom_addr.to_string(),
        },
    )
    .unwrap();

    // error cases
    // revert transfer state to correct state
    transfer.local_channel_id = local_channel.to_string();
    let receive_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: original_sender.to_string(),
        amount: Uint128::from(amount),
        msg: to_binary(&transfer).unwrap(),
    });

    // now we execute transfer back to remote chain
    let res = execute(deps.as_mut(), mock_env(), info.clone(), receive_msg).unwrap();

    assert_eq!(res.messages[0].gas_limit, None);
    println!("res messages: {:?}", res.messages);
    assert_eq!(res.messages.len(), 2); // 2 because it also has deduct fee msg
    match res.messages[1].msg.clone() {
        CosmosMsg::Ibc(IbcMsg::SendPacket {
            channel_id,
            data,
            timeout,
        }) => {
            let expected_timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
            assert_eq!(timeout, expected_timeout.into());
            assert_eq!(channel_id.as_str(), local_channel);
            let msg: Ics20Packet = from_binary(&data).unwrap();
            assert_eq!(
                msg.amount,
                Uint128::new(1234567).sub(Uint128::from(fee_amount))
            );
            assert_eq!(
                msg.denom.as_str(),
                get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
            );
            assert_eq!(msg.sender.as_str(), original_sender);
            assert_eq!(msg.receiver.as_str(), remote_address);
            // assert_eq!(msg.memo, None);
        }
        _ => panic!("Unexpected return message: {:?}", res.messages[0]),
    }
    match res.messages[0].msg.clone() {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds: _,
        }) => {
            assert_eq!(contract_addr, token_addr.to_string());
            assert_eq!(
                msg,
                to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "gov".to_string(),
                    amount: fee_amount.clone()
                })
                .unwrap()
            );
        }
        _ => panic!("Unexpected return message: {:?}", res.messages[0]),
    }

    // check new channel state after reducing balance
    let chan = query_channel(deps.as_ref(), local_channel.into()).unwrap();
    assert_eq!(
        chan.balances,
        vec![Amount::native(
            fee_amount.u128(),
            &get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
        )]
    );
    assert_eq!(
        chan.total_sent,
        vec![Amount::native(
            amount,
            &get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
        )]
    );

    // mapping pair error with wrong voucher denom
    let pair = UpdatePairMsg {
        local_channel_id: "not_registered_channel".to_string(),
        denom: denom.to_string(),
        local_asset_info: AssetInfo::Token {
            contract_addr: Addr::unchecked("random_cw20_denom".to_string()),
        },
        remote_decimals: 18u8,
        local_asset_info_decimals: 18u8,
    };

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("gov", &[]),
        ExecuteMsg::UpdateMappingPair(pair),
    )
    .unwrap();

    transfer.local_channel_id = "not_registered_channel".to_string();
    let invalid_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: original_sender.to_string(),
        amount: Uint128::from(amount),
        msg: to_binary(&transfer).unwrap(),
    });
    let err = execute(deps.as_mut(), mock_env(), info.clone(), invalid_msg).unwrap_err();
    assert_eq!(err, ContractError::MappingPairNotFound {});
}

#[test]
fn send_from_remote_to_local_receive_happy_path() {
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
    let relayer = Addr::unchecked("orai12zyu8w93h0q2lcnt50g3fn0w3yqnhy4fvawaqz");
    let send_channel = "channel-9";
    let custom_addr = "orai12zyu8w93h0q2lcnt50g3fn0w3yqnhy4fvawaqz";
    let denom = "uatom0x";
    let asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked(cw20_addr),
    };
    let contract_port = format!("wasm.{}", CONTRACT);
    let gas_limit = 1234567;
    let send_amount = Uint128::from(876543210u64);
    let channels = &["channel-1", "channel-7", send_channel];

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

    for channel_id in channels {
        let channel = mock_channel(channel_id);
        let open_msg = IbcChannelOpenMsg::new_init(channel.clone());
        contract_instance.ibc_channel_open(open_msg).unwrap();
        let connect_msg = IbcChannelConnectMsg::new_ack(channel, ICS20_VERSION);
        contract_instance.ibc_channel_connect(connect_msg).unwrap();
    }

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
    };

    contract_instance
        .execute(ExecuteMsg::UpdateMappingPair(pair), SENDER, &[])
        .unwrap();

    let data = Ics20Packet {
        // this is returning a foreign native token, thus denom is <denom>, eg: uatom
        denom: denom.to_string(),
        amount: send_amount,
        sender: SENDER.to_string(),
        receiver: custom_addr.to_string(),
        memo: None,
    };
    let recv_packet = IbcPacket::new(
        to_binary(&data).unwrap(),
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: "channel-1234".to_string(),
        },
        IbcEndpoint {
            port_id: contract_port.clone(),
            channel_id: send_channel.to_string(),
        },
        3,
        Timestamp::from_seconds(1665321069).into(),
    );

    // we can receive this denom, channel balance should increase
    let ibc_msg = IbcPacketReceiveMsg::new(recv_packet.clone(), relayer);

    let (res, _gas_used) = contract_instance.ibc_packet_receive(ibc_msg).unwrap();

    // TODO: fix test cases. Possibly because we are adding two add_submessages?
    assert_eq!(res.messages.len(), 3); // 3 messages because we also have deduct fee msg and increase channel balance msg
    match res.messages[0].msg.clone() {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds: _,
        }) => {
            assert_eq!(contract_addr, cw20_addr);
            assert_eq!(
                msg,
                to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: SENDER.to_string(),
                    amount: Uint128::from(87654321u64) // send amount / token fee
                })
                .unwrap()
            );
        }
        _ => panic!("Unexpected return message: {:?}", res.messages[0]),
    }

    let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
    assert!(matches!(ack, Ics20Ack::Result(_)));

    // query channel state|_|
    match res.messages[1].msg.clone() {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr,
            msg,
            funds: _,
        }) => {
            assert_eq!(contract_addr, CONTRACT); // self-call msg
            assert_eq!(
                msg,
                to_binary(&ExecuteMsg::IncreaseChannelBalanceIbcReceive {
                    dest_channel_id: send_channel.to_string(),
                    ibc_denom: get_key_ics20_ibc_denom(contract_port.as_str(), send_channel, denom),
                    amount: send_amount,
                    local_receiver: custom_addr.to_string(),
                })
                .unwrap()
            );
        }
        _ => panic!("Unexpected return message: {:?}", res.messages[0]),
    }
}

#[test]
fn test_swap_operations() {
    let receiver_asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("contract"),
    };
    let mut initial_asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("addr"),
    };
    let fee_denom = "orai".to_string();

    let operations = build_swap_operations(
        receiver_asset_info.clone(),
        initial_asset_info.clone(),
        fee_denom.as_str(),
    );
    assert_eq!(operations.len(), 2);

    let fee_denom = "contract".to_string();
    let operations = build_swap_operations(
        receiver_asset_info.clone(),
        initial_asset_info.clone(),
        &fee_denom,
    );
    assert_eq!(operations.len(), 1);
    assert_eq!(
        operations[0],
        SwapOperation::OraiSwap {
            offer_asset_info: initial_asset_info.clone(),
            ask_asset_info: AssetInfo::NativeToken {
                denom: fee_denom.clone()
            }
        }
    );
    initial_asset_info = AssetInfo::NativeToken {
        denom: "contract".to_string(),
    };
    let operations = build_swap_operations(
        receiver_asset_info.clone(),
        initial_asset_info.clone(),
        &fee_denom,
    );
    assert_eq!(operations.len(), 0);

    initial_asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("addr"),
    };
    let operations = build_swap_operations(
        receiver_asset_info.clone(),
        initial_asset_info.clone(),
        &fee_denom,
    );
    assert_eq!(operations.len(), 1);
    assert_eq!(
        operations[0],
        SwapOperation::OraiSwap {
            offer_asset_info: initial_asset_info.clone(),
            ask_asset_info: AssetInfo::NativeToken { denom: fee_denom }
        }
    );

    // initial = receiver => build swap ops length = 0
    let operations = build_swap_operations(
        AssetInfo::NativeToken {
            denom: "foobar".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "foobar".to_string(),
        },
        "not_foo_bar",
    );
    assert_eq!(operations.len(), 0);
}

#[test]
fn test_build_swap_msgs() {
    let minimum_receive = Uint128::from(10u128);
    let swap_router_contract = "router";
    let amount = Uint128::from(100u128);
    let mut initial_receive_asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("addr"),
    };
    let native_denom = "foobar";
    let to: Option<Addr> = None;
    let mut cosmos_msgs: Vec<SubMsg> = vec![];
    let mut operations: Vec<SwapOperation> = vec![];
    build_swap_msgs(
        minimum_receive.clone(),
        &oraiswap::router::RouterController(swap_router_contract.to_string()),
        amount.clone(),
        initial_receive_asset_info.clone(),
        to.clone(),
        &mut cosmos_msgs,
        operations.clone(),
    )
    .unwrap();
    assert_eq!(cosmos_msgs.len(), 0);
    operations.push(SwapOperation::OraiSwap {
        offer_asset_info: initial_receive_asset_info.clone(),
        ask_asset_info: initial_receive_asset_info.clone(),
    });
    build_swap_msgs(
        minimum_receive.clone(),
        &oraiswap::router::RouterController(swap_router_contract.to_string()),
        amount.clone(),
        initial_receive_asset_info.clone(),
        to.clone(),
        &mut cosmos_msgs,
        operations.clone(),
    )
    .unwrap();
    // send in Cw20 send
    assert_eq!(true, format!("{:?}", cosmos_msgs[0]).contains("send"));

    // reset cosmos msg to continue testing
    cosmos_msgs.pop();
    initial_receive_asset_info = AssetInfo::NativeToken {
        denom: native_denom.to_string(),
    };
    build_swap_msgs(
        minimum_receive.clone(),
        &oraiswap::router::RouterController(swap_router_contract.to_string()),
        amount.clone(),
        initial_receive_asset_info.clone(),
        to.clone(),
        &mut cosmos_msgs,
        operations.clone(),
    )
    .unwrap();
    assert_eq!(
        true,
        format!("{:?}", cosmos_msgs[0]).contains("execute_swap_operations")
    );
    assert_eq!(
        SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: swap_router_contract.to_string(),
                msg: to_binary(&oraiswap::router::ExecuteMsg::ExecuteSwapOperations {
                    operations: operations,
                    minimum_receive: Some(minimum_receive),
                    to
                })
                .unwrap(),
                funds: coins(amount.u128(), native_denom)
            }),
            SWAP_OPS_FAILURE_ID
        ),
        cosmos_msgs[0]
    );
}

#[test]
fn test_build_swap_msgs_forbidden_case() {
    let minimum_receive = Uint128::from(10u128);
    let swap_router_contract = "router";
    let amount = Uint128::from(100u128);
    let initial_receive_asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("addr"),
    };
    let mut cosmos_msgs: Vec<SubMsg> = vec![];
    let operations: Vec<SwapOperation> = vec![SwapOperation::OraiSwap {
        offer_asset_info: initial_receive_asset_info.clone(),
        ask_asset_info: initial_receive_asset_info.clone(),
    }];
    cosmos_msgs.push(SubMsg::new(CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
        to_address: "foobar".to_string(),
        amount: coins(1u128, "orai"),
    })));
    cosmos_msgs.push(SubMsg::new(CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
        to_address: "foobar".to_string(),
        amount: coins(1u128, "orai"),
    })));
    cosmos_msgs.push(SubMsg::new(CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
        to_address: "foobar".to_string(),
        amount: coins(1u128, "orai"),
    })));
    build_swap_msgs(
        minimum_receive.clone(),
        &oraiswap::router::RouterController(swap_router_contract.to_string()),
        amount.clone(),
        initial_receive_asset_info.clone(),
        Some(Addr::unchecked("attacker")),
        &mut cosmos_msgs,
        operations.clone(),
    )
    .unwrap();
    // should pop everything since 'to' is not None, and ops have items in it
    assert_eq!(cosmos_msgs.len(), 0);
}

#[test]
fn test_get_ibc_msg_evm_case() {
    // setup
    let send_channel = "channel-9";
    let receive_channel = "channel-1";
    let allowed = "foobar";
    let pair_mapping_denom = "trx-mainnet0xa614f803B6FD780986A42c78Ec9c7f77e6DeD13C";
    let allowed_gas = 777666;
    let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);
    let receiver_asset_info = AssetInfo::NativeToken {
        denom: "orai".to_string(),
    };
    let amount = Uint128::from(10u128);
    let remote_decimals = 18;
    let asset_info_decimals = 6;
    let remote_amount =
        convert_local_to_remote(amount, remote_decimals, asset_info_decimals).unwrap();
    let remote_address = "eth-mainnet0x1235";
    let mut env = mock_env();
    env.contract.address = Addr::unchecked("addr");
    let mut destination = DestinationInfo {
        receiver: "0x1234".to_string(),
        destination_channel: "channel-10".to_string(),
        destination_denom: "atom".to_string(),
    };
    let timeout = 1000u64;
    let local_receiver = "local_receiver";

    // first case, destination channel empty
    destination.destination_channel = "".to_string();

    let err = build_ibc_msg(
        env.clone(),
        local_receiver,
        receive_channel,
        amount,
        remote_address,
        &destination,
        timeout,
        None,
    )
    .unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("Destination channel empty in build ibc msg")
    );

    // evm based case, error getting pair mapping
    destination.receiver = "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string();
    destination.destination_channel = send_channel.to_string();
    let err = build_ibc_msg(
        env.clone(),
        local_receiver,
        receive_channel,
        amount,
        remote_address,
        &destination,
        timeout,
        None,
    )
    .unwrap_err();
    assert_eq!(err, StdError::generic_err("cannot find pair mappings"));

    // add a pair mapping so we can test the happy case evm based happy case
    let update = UpdatePairMsg {
        local_channel_id: "mars-channel".to_string(),
        denom: pair_mapping_denom.to_string(),
        local_asset_info: receiver_asset_info.clone(),
        remote_decimals,
        local_asset_info_decimals: asset_info_decimals,
    };

    // works with proper funds
    let msg = ExecuteMsg::UpdateMappingPair(update.clone());

    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    let pair_mapping_key = format!(
        "wasm.{}/{}/{}",
        "cosmos2contract", update.local_channel_id, pair_mapping_denom
    );
    increase_channel_balance(
        deps.as_mut().storage,
        receive_channel,
        pair_mapping_key.as_str(),
        remote_amount.clone(),
    )
    .unwrap();
    destination.receiver = "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string();
    destination.destination_channel = update.local_channel_id;
    let result = build_ibc_msg(
        env.clone(),
        local_receiver,
        receive_channel,
        amount,
        remote_address,
        &destination,
        timeout,
        Some(PairQuery {
            key: pair_mapping_key.clone(),
            pair_mapping: MappingMetadata {
                asset_info: receiver_asset_info.clone(),
                remote_decimals,
                asset_info_decimals: asset_info_decimals.clone(),
            },
        }),
    )
    .unwrap();

    assert_eq!(
        result[1],
        SubMsg::reply_on_error(
            CosmosMsg::Ibc(IbcMsg::SendPacket {
                channel_id: receive_channel.to_string(),
                data: to_binary(&Ics20Packet::new(
                    remote_amount.clone(),
                    pair_mapping_key.clone(),
                    env.contract.address.as_str(),
                    &remote_address,
                    Some(destination.receiver),
                ))
                .unwrap(),
                timeout: env.block.time.plus_seconds(timeout).into()
            }),
            FOLLOW_UP_IBC_SEND_FAILURE_ID
        )
    );
    assert_eq!(
        result[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.into_string(),
            msg: to_binary(&ExecuteMsg::ReduceChannelBalanceIbcReceive {
                src_channel_id: receive_channel.to_string(),
                ibc_denom: pair_mapping_key,
                amount: remote_amount,
                local_receiver: local_receiver.to_string()
            })
            .unwrap(),
            funds: vec![]
        }))
    );
}

#[test]
fn test_get_ibc_msg_cosmos_based_case() {
    // setup
    let send_channel = "channel-10";
    let allowed = "foobar";
    let allowed_gas = 777666;
    let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);
    let amount = Uint128::from(1000u64);
    let pair_mapping_denom = "cosmos1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n";
    let receiver_asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("usdt"),
    };
    let local_channel_id = "channel";
    let local_receiver = "receiver";
    let timeout = 10u64;
    let remote_amount = convert_local_to_remote(amount.clone(), 18, 6).unwrap();
    let destination = DestinationInfo {
        receiver: "cosmos1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".to_string(),
        destination_channel: send_channel.to_string(),
        destination_denom: "atom".to_string(),
    };
    let env = mock_env();
    let remote_address = "foobar";
    let ibc_denom = format!("foo/bar/{}", pair_mapping_denom);
    let remote_decimals = 18;
    let asset_info_decimals = 6;
    let pair_mapping_key = format!(
        "wasm.cosmos2contract/{}/{}",
        send_channel, pair_mapping_denom
    );

    CHANNEL_REVERSE_STATE
        .save(
            deps.as_mut().storage,
            (local_channel_id, ibc_denom.as_str()),
            &ChannelState {
                outstanding: remote_amount.clone(),
                total_sent: Uint128::from(100u128),
            },
        )
        .unwrap();

    CHANNEL_REVERSE_STATE
        .save(
            deps.as_mut().storage,
            (send_channel, pair_mapping_key.as_str()),
            &ChannelState {
                outstanding: remote_amount.clone(),
                total_sent: Uint128::from(100u128),
            },
        )
        .unwrap();

    // cosmos based case but no mapping found. should be successful & cosmos msg is ibc transfer
    let result = build_ibc_msg(
        env.clone(),
        local_receiver,
        local_channel_id,
        amount,
        remote_address,
        &destination,
        timeout,
        None,
    )
    .unwrap();
    assert_eq!(
        result[0],
        SubMsg::reply_on_error(
            CosmosMsg::Ibc(IbcMsg::Transfer {
                channel_id: send_channel.to_string(),
                to_address: destination.receiver.clone(),
                amount: coin(1000u128, "atom"),
                timeout: mock_env().block.time.plus_seconds(timeout).into()
            }),
            IBC_TRANSFER_NATIVE_ERROR_ID
        )
    );

    // cosmos based case with mapping found. Should be successful & cosmos msg is ibc send packet
    // add a pair mapping so we can test the happy case evm based happy case
    let update = UpdatePairMsg {
        local_channel_id: send_channel.to_string(),
        denom: pair_mapping_denom.to_string(),
        local_asset_info: receiver_asset_info.clone(),
        remote_decimals,
        local_asset_info_decimals: asset_info_decimals,
    };

    let msg = ExecuteMsg::UpdateMappingPair(update.clone());

    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    CHANNEL_REVERSE_STATE
        .save(
            deps.as_mut().storage,
            (local_channel_id, &pair_mapping_key),
            &ChannelState {
                outstanding: remote_amount.clone(),
                total_sent: Uint128::from(100u128),
            },
        )
        .unwrap();

    // now we get ibc msg
    let result = build_ibc_msg(
        env.clone(),
        local_receiver,
        local_channel_id,
        amount,
        remote_address,
        &destination,
        timeout,
        Some(PairQuery {
            key: pair_mapping_key.clone(),
            pair_mapping: MappingMetadata {
                asset_info: receiver_asset_info.clone(),
                remote_decimals,
                asset_info_decimals,
            },
        }),
    )
    .unwrap();

    assert_eq!(
        result[1],
        SubMsg::reply_on_error(
            CosmosMsg::Ibc(IbcMsg::SendPacket {
                channel_id: send_channel.to_string(),
                data: to_binary(&Ics20Packet::new(
                    remote_amount.clone(),
                    pair_mapping_key.clone(),
                    env.contract.address.as_str(),
                    &destination.receiver,
                    None,
                ))
                .unwrap(),
                timeout: env.block.time.plus_seconds(timeout).into()
            }),
            FOLLOW_UP_IBC_SEND_FAILURE_ID
        )
    );
    assert_eq!(
        result[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.into_string(),
            msg: to_binary(&ExecuteMsg::ReduceChannelBalanceIbcReceive {
                src_channel_id: send_channel.to_string(),
                ibc_denom: pair_mapping_key,
                amount: remote_amount,
                local_receiver: local_receiver.to_string()
            })
            .unwrap(),
            funds: vec![]
        }))
    );
}

#[test]
fn test_get_ibc_msg_neither_cosmos_or_evm_based_case() {
    // setup
    let amount = Uint128::from(1000u64);
    let local_channel_id = "channel";
    let local_receiver = "receiver";
    let timeout = 10u64;
    let destination = DestinationInfo {
        receiver: "foo".to_string(),
        destination_channel: "channel-10".to_string(),
        destination_denom: "atom".to_string(),
    };
    let env = mock_env();
    let remote_address = "foobar";
    // cosmos based case but no mapping found. should be successful & cosmos msg is ibc transfer
    let result = build_ibc_msg(
        env.clone(),
        local_receiver,
        local_channel_id,
        amount,
        remote_address,
        &destination,
        timeout,
        None,
    )
    .unwrap_err();
    assert_eq!(
        result,
        StdError::generic_err("The destination info is neither evm or cosmos based")
    )
}

#[test]
fn test_follow_up_msgs() {
    let send_channel = "channel-9";
    let local_channel = "channel";
    let allowed = "foobar";
    let allowed_gas = 777666;
    let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);
    let deps_mut = deps.as_mut();
    let receiver = "foobar";
    let amount = Uint128::from(1u128);
    let mut env = mock_env();
    env.contract.address = Addr::unchecked("foobar");
    let initial_asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("addr"),
    };

    // first case, memo empty => return send amount with receiver input
    let result = get_follow_up_msgs(
        deps_mut.storage,
        deps_mut.api,
        &deps_mut.querier,
        env.clone(),
        Amount::Cw20(Cw20Coin {
            address: "foobar".to_string(),
            amount: amount.clone(),
        }),
        initial_asset_info.clone(),
        AssetInfo::NativeToken {
            denom: "".to_string(),
        },
        "foobar",
        receiver,
        &DestinationInfo::from_str(""),
        local_channel,
        None,
    )
    .unwrap();

    assert_eq!(
        result.sub_msgs,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: receiver.to_string(),
                    amount: amount.clone()
                })
                .unwrap(),
                funds: vec![]
            }),
            NATIVE_RECEIVE_ID
        )]
    );

    // 2nd case, destination denom is empty => destination is collected from memo
    let memo = "channel-15/cosmosabcd";
    let result = get_follow_up_msgs(
        deps_mut.storage,
        deps_mut.api,
        &deps_mut.querier,
        env.clone(),
        Amount::Cw20(Cw20Coin {
            address: "foobar".to_string(),
            amount,
        }),
        initial_asset_info.clone(),
        AssetInfo::NativeToken {
            denom: "cosmosabcd".to_string(),
        },
        "foobar",
        "foobar",
        &DestinationInfo::from_str(memo),
        local_channel,
        None,
    )
    .unwrap();

    assert_eq!(
        result.sub_msgs,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: receiver.to_string(),
                    amount: amount.clone()
                })
                .unwrap(),
                funds: vec![]
            }),
            NATIVE_RECEIVE_ID
        )]
    );

    // 3rd case, cosmos msgs empty case, also send amount
    let memo = "cosmosabcd:orai";
    let result = get_follow_up_msgs(
        deps_mut.storage,
        deps_mut.api,
        &deps_mut.querier,
        env.clone(),
        Amount::Cw20(Cw20Coin {
            address: "foobar".to_string(),
            amount,
        }),
        AssetInfo::NativeToken {
            denom: "orai".to_string(),
        },
        AssetInfo::NativeToken {
            denom: "orai".to_string(),
        },
        "foobar",
        "foobar",
        &DestinationInfo::from_str(memo),
        local_channel,
        None,
    )
    .unwrap();

    assert_eq!(
        result.sub_msgs,
        vec![SubMsg::reply_on_error(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: receiver.to_string(),
                    amount: amount.clone()
                })
                .unwrap(),
                funds: vec![]
            }),
            NATIVE_RECEIVE_ID
        )]
    );
}

#[test]
fn test_deduct_fee() {
    assert_eq!(
        deduct_fee(
            Ratio {
                nominator: 1,
                denominator: 0,
            },
            Uint128::from(1000u64)
        ),
        Uint128::from(0u64)
    );
    assert_eq!(
        deduct_fee(
            Ratio {
                nominator: 1,
                denominator: 1,
            },
            Uint128::from(1000u64)
        ),
        Uint128::from(1000u64)
    );
    assert_eq!(
        deduct_fee(
            Ratio {
                nominator: 1,
                denominator: 100,
            },
            Uint128::from(1000u64)
        ),
        Uint128::from(10u64)
    );
}

#[test]
fn test_convert_remote_denom_to_evm_prefix() {
    assert_eq!(convert_remote_denom_to_evm_prefix("abcd"), "".to_string());
    assert_eq!(convert_remote_denom_to_evm_prefix("0x"), "".to_string());
    assert_eq!(
        convert_remote_denom_to_evm_prefix("evm0x"),
        "evm".to_string()
    );
}

#[test]
fn test_parse_ibc_denom_without_sanity_checks() {
    assert_eq!(parse_ibc_denom_without_sanity_checks("foo").is_err(), true);
    assert_eq!(
        parse_ibc_denom_without_sanity_checks("foo/bar").is_err(),
        true
    );
    let result = parse_ibc_denom_without_sanity_checks("foo/bar/helloworld").unwrap();
    assert_eq!(result, "helloworld");

    let result = parse_ibc_info_without_sanity_checks("foo/bar").unwrap_or_default();
    assert_eq!(result.0, "");
    assert_eq!(result.1, "");
    assert_eq!(result.2, "");
}

#[test]
fn test_parse_ibc_channel_without_sanity_checks() {
    assert_eq!(
        parse_ibc_channel_without_sanity_checks("foo").is_err(),
        true
    );
    assert_eq!(
        parse_ibc_channel_without_sanity_checks("foo/bar").is_err(),
        true
    );
    let result = parse_ibc_channel_without_sanity_checks("foo/bar/helloworld").unwrap();
    assert_eq!(result, "bar");

    let result = parse_ibc_info_without_sanity_checks("foo/bar").unwrap_or_default();
    assert_eq!(result.0, "");
    assert_eq!(result.1, "");
    assert_eq!(result.2, "");
}

#[test]
fn test_parse_ibc_info_without_sanity_checks() {
    assert_eq!(parse_ibc_info_without_sanity_checks("foo").is_err(), true);
    assert_eq!(
        parse_ibc_info_without_sanity_checks("foo/bar").is_err(),
        true
    );
    let result = parse_ibc_info_without_sanity_checks("foo/bar/helloworld").unwrap();
    assert_eq!(result.0, "foo");
    assert_eq!(result.1, "bar");
    assert_eq!(result.2, "helloworld");

    let result = parse_ibc_info_without_sanity_checks("foo/bar").unwrap_or_default();
    assert_eq!(result.0, "");
    assert_eq!(result.1, "");
    assert_eq!(result.2, "");
}

#[test]
fn test_deduct_token_fee() {
    let mut deps = mock_dependencies();
    let amount = Uint128::from(1000u64);
    let storage = deps.as_mut().storage;
    let token_fee_denom = "foo0x";
    // should return amount because we have not set relayer fee yet
    assert_eq!(
        deduct_token_fee(storage, "foo", amount).unwrap().0,
        amount.clone()
    );
    TOKEN_FEE
        .save(
            storage,
            token_fee_denom,
            &Ratio {
                nominator: 1,
                denominator: 100,
            },
        )
        .unwrap();
    assert_eq!(
        deduct_token_fee(storage, token_fee_denom, amount)
            .unwrap()
            .0,
        Uint128::from(990u64)
    );
}

#[test]
fn test_deduct_relayer_fee() {
    let mut deps = mock_dependencies();
    let deps_mut = deps.as_mut();
    let token_fee_denom = "cosmos";
    let remote_address = "cosmos1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n";
    let offer_amount = Uint128::from(10u32.pow(0 as u32));
    let token_price = Uint128::from(10u64);
    // token price empty case. Should return zero fee
    let result = deduct_relayer_fee(
        deps_mut.storage,
        deps_mut.api,
        remote_address,
        token_fee_denom,
        offer_amount.clone(),
        Uint128::from(0u64),
    )
    .unwrap();
    assert_eq!(result, Uint128::from(0u64));

    // remote address is wrong (dont follow bech32 form)
    assert_eq!(
        deduct_relayer_fee(
            deps_mut.storage,
            deps_mut.api,
            "foobar",
            token_fee_denom,
            offer_amount.clone(),
            token_price,
        )
        .unwrap(),
        Uint128::from(0u128)
    );

    // no relayer fee case
    assert_eq!(
        deduct_relayer_fee(
            deps_mut.storage,
            deps_mut.api,
            remote_address,
            token_fee_denom,
            offer_amount.clone(),
            token_price,
        )
        .unwrap(),
        Uint128::from(0u64)
    );

    // oraib prefix case.
    RELAYER_FEE
        .save(deps_mut.storage, token_fee_denom, &Uint128::from(100u64))
        .unwrap();

    RELAYER_FEE
        .save(deps_mut.storage, "foo", &Uint128::from(1000u64))
        .unwrap();

    assert_eq!(
        deduct_relayer_fee(
            deps_mut.storage,
            deps_mut.api,
            "oraib1603j3e4juddh7cuhfquxspl0p0nsun047wz3rl",
            "foo0x",
            offer_amount.clone(),
            token_price,
        )
        .unwrap(),
        Uint128::from(100u64)
    );

    // normal case with remote address
    assert_eq!(
        deduct_relayer_fee(
            deps_mut.storage,
            deps_mut.api,
            remote_address,
            token_fee_denom,
            offer_amount.clone(),
            token_price,
        )
        .unwrap(),
        Uint128::from(10u64)
    );
}

#[test]
fn test_process_ibc_msg() {
    // setup
    let mut deps = mock_dependencies();
    let amount = Uint128::from(1000u64);
    let storage = deps.as_mut().storage;
    let ibc_denom = "foo/bar/cosmos";
    let pair_mapping = PairQuery {
        key: ibc_denom.to_string(),
        pair_mapping: MappingMetadata {
            asset_info: AssetInfo::NativeToken {
                denom: "orai".to_string(),
            },
            remote_decimals: 18,
            asset_info_decimals: 6,
        },
    };
    let local_channel_id = "channel";
    let ibc_msg_sender = "sender";
    let ibc_msg_receiver = "receiver";
    let local_receiver = "local_receiver";
    let memo = None;
    let timeout = Timestamp::from_seconds(10u64);
    let remote_amount = convert_local_to_remote(amount.clone(), 18, 6).unwrap();

    CHANNEL_REVERSE_STATE
        .save(
            storage,
            (local_channel_id, ibc_denom),
            &ChannelState {
                outstanding: remote_amount.clone(),
                total_sent: Uint128::from(100u128),
            },
        )
        .unwrap();

    // action
    let result = process_ibc_msg(
        pair_mapping,
        mock_env().contract.address.into_string(),
        local_receiver,
        local_channel_id,
        ibc_msg_sender,
        ibc_msg_receiver,
        memo,
        amount,
        timeout,
    )
    .unwrap();

    assert_eq!(
        result[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mock_env().contract.address.into_string(),
            msg: to_binary(&ExecuteMsg::ReduceChannelBalanceIbcReceive {
                src_channel_id: local_channel_id.to_string(),
                ibc_denom: ibc_denom.to_string(),
                amount: remote_amount,
                local_receiver: local_receiver.to_string()
            })
            .unwrap(),
            funds: vec![]
        }))
    );

    assert_eq!(
        result[1],
        SubMsg::reply_on_error(
            IbcMsg::SendPacket {
                channel_id: local_channel_id.to_string(),
                data: to_binary(&Ics20Packet {
                    amount: remote_amount.clone(),
                    denom: ibc_denom.to_string(),
                    receiver: ibc_msg_receiver.to_string(),
                    sender: ibc_msg_sender.to_string(),
                    memo: None
                })
                .unwrap(),
                timeout: IbcTimeout::with_timestamp(timeout)
            },
            FOLLOW_UP_IBC_SEND_FAILURE_ID
        )
    )
}

#[test]
fn test_get_token_price_orai_case() {
    let deps = mock_dependencies();
    let simulate_amount = Uint128::from(10u128);
    let result = get_token_price(
        &deps.as_ref().querier,
        simulate_amount,
        &RouterController("foo".to_string()),
        AssetInfo::NativeToken {
            denom: "orai".to_string(),
        },
    );
    assert_eq!(result, simulate_amount)
}

#[test]
fn test_split_denom() {
    let split_denom: Vec<&str> = "orai".splitn(3, '/').collect();
    assert_eq!(split_denom.len(), 1);

    let split_denom: Vec<&str> = "a/b/c".splitn(3, '/').collect();
    assert_eq!(split_denom.len(), 3)
}

#[test]
fn setup_and_query() {
    let deps = setup(&["channel-3", "channel-7"], &[]);

    let raw_list = query(deps.as_ref(), mock_env(), QueryMsg::ListChannels {}).unwrap();
    let list_res: ListChannelsResponse = from_binary(&raw_list).unwrap();
    assert_eq!(2, list_res.channels.len());
    assert_eq!(mock_channel_info("channel-3"), list_res.channels[0]);
    assert_eq!(mock_channel_info("channel-7"), list_res.channels[1]);

    let raw_channel = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Channel {
            id: "channel-3".to_string(),
        },
    )
    .unwrap();
    let chan_res: ChannelResponse = from_binary(&raw_channel).unwrap();
    assert_eq!(chan_res.info, mock_channel_info("channel-3"));
    assert_eq!(0, chan_res.total_sent.len());
    assert_eq!(0, chan_res.balances.len());

    let err = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Channel {
            id: "channel-10".to_string(),
        },
    )
    .unwrap_err();
    assert_eq!(err, StdError::not_found("cw_ics20::state::ChannelInfo"));
}

#[test]
fn test_query_pair_mapping_by_asset_info() {
    let mut deps = setup(&["channel-3", "channel-7"], &[]);
    let asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("cw20:foobar".to_string()),
    };
    let mut update = UpdatePairMsg {
        local_channel_id: "mars-channel".to_string(),
        denom: "earth".to_string(),
        local_asset_info: asset_info.clone(),
        remote_decimals: 18,
        local_asset_info_decimals: 18,
    };

    // works with proper funds
    let mut msg = ExecuteMsg::UpdateMappingPair(update.clone());

    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // add another pair with the same asset info to filter
    update.denom = "jupiter".to_string();
    msg = ExecuteMsg::UpdateMappingPair(update.clone());
    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // add another pair with a different asset info
    update.denom = "moon".to_string();
    update.local_asset_info = AssetInfo::NativeToken {
        denom: "orai".to_string(),
    };
    msg = ExecuteMsg::UpdateMappingPair(update.clone());
    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // query based on asset info

    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappingsFromAssetInfo {
            asset_info: asset_info,
        },
    )
    .unwrap();
    let response: Vec<PairQuery> = from_binary(&mappings).unwrap();
    assert_eq!(response.len(), 2);

    // query native token asset info, should receive moon denom in key
    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappingsFromAssetInfo {
            asset_info: AssetInfo::NativeToken {
                denom: "orai".to_string(),
            },
        },
    )
    .unwrap();
    let response: Vec<PairQuery> = from_binary(&mappings).unwrap();
    assert_eq!(response.len(), 1);
    assert_eq!(response.first().unwrap().key.contains("moon"), true);

    // query asset info that is not in the mapping, should return empty
    // query native token asset info, should receive moon denom
    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappingsFromAssetInfo {
            asset_info: AssetInfo::NativeToken {
                denom: "foobar".to_string(),
            },
        },
    )
    .unwrap();
    let response: Vec<PairQuery> = from_binary(&mappings).unwrap();
    assert_eq!(response.len(), 0);
}

#[test]
fn test_update_cw20_mapping() {
    let mut deps = setup(&["channel-3", "channel-7"], &[]);
    let asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("cw20:foobar".to_string()),
    };
    let asset_info_second = AssetInfo::Token {
        contract_addr: Addr::unchecked("cw20:foobar-second".to_string()),
    };

    let mut update = UpdatePairMsg {
        local_channel_id: "mars-channel".to_string(),
        denom: "earth".to_string(),
        local_asset_info: asset_info.clone(),
        remote_decimals: 18,
        local_asset_info_decimals: 18,
    };

    // works with proper funds
    let mut msg = ExecuteMsg::UpdateMappingPair(update.clone());

    // unauthorized case
    let info = mock_info("foobar", &coins(1234567, "ucosm"));
    let res_err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(res_err, ContractError::Admin(AdminError::NotAdmin {}));

    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // query to verify if the mapping has been updated
    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappings {
            start_after: None,
            limit: None,
            order: None,
        },
    )
    .unwrap();
    let response: ListMappingResponse = from_binary(&mappings).unwrap();
    println!("response: {:?}", response);
    assert_eq!(
        response.pairs.first().unwrap().key,
        format!("{}/mars-channel/earth", CONTRACT_PORT)
    );

    // not found case
    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappings {
            start_after: None,
            limit: None,
            order: None,
        },
    )
    .unwrap();
    let response: ListMappingResponse = from_binary(&mappings).unwrap();
    assert_ne!(response.pairs.first().unwrap().key, "foobar".to_string());

    // update existing key case must pass
    update.local_asset_info = asset_info_second.clone();
    msg = ExecuteMsg::UpdateMappingPair(update.clone());

    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // after update, cw20 denom now needs to be updated
    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappings {
            start_after: None,
            limit: None,
            order: None,
        },
    )
    .unwrap();
    let response: ListMappingResponse = from_binary(&mappings).unwrap();
    println!("response: {:?}", response);
    assert_eq!(
        response.pairs.first().unwrap().key,
        format!("{}/mars-channel/earth", CONTRACT_PORT)
    );
    assert_eq!(
        response.pairs.first().unwrap().pair_mapping.asset_info,
        asset_info_second
    )
}

#[test]
fn test_delete_cw20_mapping() {
    let mut deps = setup(&["channel-3", "channel-7"], &[]);
    let cw20_denom = AssetInfo::Token {
        contract_addr: Addr::unchecked("cw20:foobar".to_string()),
    };

    let update = UpdatePairMsg {
        local_channel_id: "mars-channel".to_string(),
        denom: "earth".to_string(),
        local_asset_info: cw20_denom.clone(),
        remote_decimals: 18,
        local_asset_info_decimals: 18,
    };

    // works with proper funds
    let msg = ExecuteMsg::UpdateMappingPair(update.clone());

    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // query to verify if the mapping has been updated
    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappings {
            start_after: None,
            limit: None,
            order: None,
        },
    )
    .unwrap();
    let response: ListMappingResponse = from_binary(&mappings).unwrap();
    println!("response: {:?}", response);
    assert_eq!(
        response.pairs.first().unwrap().key,
        format!("{}/mars-channel/earth", CONTRACT_PORT)
    );

    // now try deleting
    let delete = DeletePairMsg {
        local_channel_id: "mars-channel".to_string(),
        denom: "earth".to_string(),
    };

    let mut msg = ExecuteMsg::DeleteMappingPair(delete.clone());

    // unauthorized delete case
    let info = mock_info("foobar", &coins(1234567, "ucosm"));
    let delete_err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
    assert_eq!(delete_err, ContractError::Admin(AdminError::NotAdmin {}));

    let info = mock_info("gov", &coins(1234567, "ucosm"));

    // happy case
    msg = ExecuteMsg::DeleteMappingPair(delete.clone());
    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    // after update, the list cw20 mapping should be empty
    let mappings = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::PairMappings {
            start_after: None,
            limit: None,
            order: None,
        },
    )
    .unwrap();
    let response: ListMappingResponse = from_binary(&mappings).unwrap();
    println!("response: {:?}", response);
    assert_eq!(response.pairs.len(), 0)
}

// #[test]
// fn proper_checks_on_execute_native() {
//     let send_channel = "channel-5";
//     let mut deps = setup(&[send_channel, "channel-10"], &[]);

//     let mut transfer = TransferMsg {
//         channel: send_channel.to_string(),
//         remote_address: "foreign-address".to_string(),
//         timeout: None,
//         memo: Some("memo".to_string()),
//     };

//     // works with proper funds
//     let msg = ExecuteMsg::Transfer(transfer.clone());
//     let info = mock_info("foobar", &coins(1234567, "ucosm"));
//     let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
//     assert_eq!(res.messages[0].gas_limit, None);
//     assert_eq!(1, res.messages.len());
//     if let CosmosMsg::Ibc(IbcMsg::SendPacket {
//         channel_id,
//         data,
//         timeout,
//     }) = &res.messages[0].msg
//     {
//         let expected_timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
//         assert_eq!(timeout, &expected_timeout.into());
//         assert_eq!(channel_id.as_str(), send_channel);
//         let msg: Ics20Packet = from_binary(data).unwrap();
//         assert_eq!(msg.amount, Uint128::new(1234567));
//         assert_eq!(msg.denom.as_str(), "ucosm");
//         assert_eq!(msg.sender.as_str(), "foobar");
//         assert_eq!(msg.receiver.as_str(), "foreign-address");
//     } else {
//         panic!("Unexpected return message: {:?}", res.messages[0]);
//     }

//     // reject with no funds
//     let msg = ExecuteMsg::Transfer(transfer.clone());
//     let info = mock_info("foobar", &[]);
//     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
//     assert_eq!(err, ContractError::Payment(PaymentError::NoFunds {}));

//     // reject with multiple tokens funds
//     let msg = ExecuteMsg::Transfer(transfer.clone());
//     let info = mock_info("foobar", &[coin(1234567, "ucosm"), coin(54321, "uatom")]);
//     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
//     assert_eq!(err, ContractError::Payment(PaymentError::MultipleDenoms {}));

//     // reject with bad channel id
//     transfer.channel = "channel-45".to_string();
//     let msg = ExecuteMsg::Transfer(transfer);
//     let info = mock_info("foobar", &coins(1234567, "ucosm"));
//     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
//     assert_eq!(
//         err,
//         ContractError::NoSuchChannel {
//             id: "channel-45".to_string()
//         }
//     );
// }

// #[test]
// fn proper_checks_on_execute_cw20() {
//     let send_channel = "channel-15";
//     let cw20_addr = "my-token";
//     let mut deps = setup(&["channel-3", send_channel], &[(cw20_addr, 123456)]);

//     let transfer = TransferMsg {
//         channel: send_channel.to_string(),
//         remote_address: "foreign-address".to_string(),
//         timeout: Some(7777),
//         memo: Some("memo".to_string()),
//     };
//     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
//         sender: "my-account".into(),
//         amount: Uint128::new(888777666),
//         msg: to_binary(&transfer).unwrap(),
//     });

//     // works with proper funds
//     let info = mock_info(cw20_addr, &[]);
//     let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
//     assert_eq!(1, res.messages.len());
//     assert_eq!(res.messages[0].gas_limit, None);
//     if let CosmosMsg::Ibc(IbcMsg::SendPacket {
//         channel_id,
//         data,
//         timeout,
//     }) = &res.messages[0].msg
//     {
//         let expected_timeout = mock_env().block.time.plus_seconds(7777);
//         assert_eq!(timeout, &expected_timeout.into());
//         assert_eq!(channel_id.as_str(), send_channel);
//         let msg: Ics20Packet = from_binary(data).unwrap();
//         assert_eq!(msg.amount, Uint128::new(888777666));
//         assert_eq!(msg.denom, format!("cw20:{}", cw20_addr));
//         assert_eq!(msg.sender.as_str(), "my-account");
//         assert_eq!(msg.receiver.as_str(), "foreign-address");
//     } else {
//         panic!("Unexpected return message: {:?}", res.messages[0]);
//     }

//     // reject with tokens funds
//     let info = mock_info("foobar", &coins(1234567, "ucosm"));
//     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
//     assert_eq!(err, ContractError::Payment(PaymentError::NonPayable {}));
// }

// #[test]
// fn execute_cw20_fails_if_not_whitelisted_unless_default_gas_limit() {
//     let send_channel = "channel-15";
//     let mut deps = setup(&[send_channel], &[]);

//     let cw20_addr = "my-token";
//     let transfer = TransferMsg {
//         channel: send_channel.to_string(),
//         remote_address: "foreign-address".to_string(),
//         timeout: Some(7777),
//         memo: Some("memo".to_string()),
//     };
//     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
//         sender: "my-account".into(),
//         amount: Uint128::new(888777666),
//         msg: to_binary(&transfer).unwrap(),
//     });

//     // rejected as not on allow list
//     let info = mock_info(cw20_addr, &[]);
//     let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
//     assert_eq!(err, ContractError::NotOnAllowList);

//     // add a default gas limit
//     migrate(
//         deps.as_mut(),
//         mock_env(),
//         MigrateMsg {
//             default_gas_limit: Some(123456),
//             fee_receiver: "receiver".to_string(),
//             default_timeout: 100u64,
//             fee_denom: "orai".to_string(),
//             swap_router_contract: "foobar".to_string(),
//         },
//     )
//     .unwrap();

//     // try again
//     execute(deps.as_mut(), mock_env(), info, msg).unwrap();
// }
// test execute transfer back to native remote chain

fn mock_receive_packet(
    remote_channel: &str,
    local_channel: &str,
    amount: u128,
    denom: &str,
    receiver: &str,
) -> IbcPacket {
    let data = Ics20Packet {
        // this is returning a foreign (our) token, thus denom is <port>/<channel>/<denom>
        denom: denom.to_string(),
        amount: amount.into(),
        sender: "remote-sender".to_string(),
        receiver: receiver.to_string(),
        memo: Some("memo".to_string()),
    };
    IbcPacket::new(
        to_binary(&data).unwrap(),
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: remote_channel.to_string(),
        },
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: local_channel.to_string(),
        },
        3,
        Timestamp::from_seconds(1665321069).into(),
    )
}

#[test]
fn proper_checks_on_execute_cw20_transfer_back_to_remote() {
    // arrange
    let relayer = Addr::unchecked("relayer");
    let remote_channel = "channel-5";
    let remote_address = "cosmos1603j3e4juddh7cuhfquxspl0p0nsun046us7n0";
    let custom_addr = "custom-addr";
    let original_sender = "original_sender";
    let denom = "uatom0x";
    let amount = 1234567u128;
    let asset_info = AssetInfo::NativeToken {
        denom: denom.into(),
    };
    let cw20_raw_denom = original_sender;
    let local_channel = "channel-1234";
    let ibc_denom = get_key_ics20_ibc_denom("wasm.cosmos2contract", local_channel, denom);
    let ratio = Ratio {
        nominator: 1,
        denominator: 10,
    };
    let fee_amount =
        Uint128::from(amount) * Decimal::from_ratio(ratio.nominator, ratio.denominator);
    let mut deps = setup(&[remote_channel, local_channel], &[]);
    TOKEN_FEE
        .save(deps.as_mut().storage, denom, &ratio)
        .unwrap();

    let pair = UpdatePairMsg {
        local_channel_id: local_channel.to_string(),
        denom: denom.to_string(),
        local_asset_info: asset_info.clone(),
        remote_decimals: 18u8,
        local_asset_info_decimals: 18u8,
    };

    let _ = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("gov", &[]),
        ExecuteMsg::UpdateMappingPair(pair),
    )
    .unwrap();

    // execute
    let mut transfer = TransferBackMsg {
        local_channel_id: local_channel.to_string(),
        remote_address: remote_address.to_string(),
        remote_denom: denom.to_string(),
        timeout: Some(DEFAULT_TIMEOUT),
        memo: None,
    };

    let msg = ExecuteMsg::TransferToRemote(transfer.clone());

    // insufficient funds case because we need to receive from remote chain first
    let info = mock_info(cw20_raw_denom, &[coin(amount, denom)]);
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());
    assert_eq!(
        res.unwrap_err(),
        ContractError::NoSuchChannelState {
            id: local_channel.to_string(),
            denom: ibc_denom.clone()
        }
    );

    // prepare some mock packets
    let recv_packet =
        mock_receive_packet(remote_channel, local_channel, amount, denom, custom_addr);

    // receive some tokens. Assume that the function works perfectly because the test case is elsewhere
    let ibc_msg = IbcPacketReceiveMsg::new(recv_packet.clone(), relayer);
    ibc_packet_receive(deps.as_mut(), mock_env(), ibc_msg).unwrap();
    // need to trigger increase channel balance because it is executed through submsg
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(mock_env().contract.address.as_str(), &[]),
        ExecuteMsg::IncreaseChannelBalanceIbcReceive {
            dest_channel_id: local_channel.to_string(),
            ibc_denom: ibc_denom.clone(),
            amount: Uint128::from(amount),
            local_receiver: custom_addr.to_string(),
        },
    )
    .unwrap();

    // error cases
    // revert transfer state to correct state
    transfer.local_channel_id = local_channel.to_string();
    let msg: ExecuteMsg = ExecuteMsg::TransferToRemote(transfer.clone());

    // now we execute transfer back to remote chain
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    assert_eq!(res.messages[0].gas_limit, None);
    println!("res messages: {:?}", res.messages);
    assert_eq!(2, res.messages.len()); // 2 because it also has deduct fee msg
    match res.messages[1].msg.clone() {
        CosmosMsg::Ibc(IbcMsg::SendPacket {
            channel_id,
            data,
            timeout,
        }) => {
            let expected_timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
            assert_eq!(timeout, expected_timeout.into());
            assert_eq!(channel_id.as_str(), local_channel);
            let msg: Ics20Packet = from_binary(&data).unwrap();
            assert_eq!(
                msg.amount,
                Uint128::new(1234567).sub(Uint128::from(fee_amount))
            );
            assert_eq!(
                msg.denom.as_str(),
                get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
            );
            assert_eq!(msg.sender.as_str(), original_sender);
            assert_eq!(msg.receiver.as_str(), remote_address);
            // assert_eq!(msg.memo, None);
        }
        _ => panic!("Unexpected return message: {:?}", res.messages[0]),
    }
    match res.messages[0].msg.clone() {
        CosmosMsg::Bank(BankMsg::Send {
            to_address,
            amount: message_amount,
        }) => {
            assert_eq!(to_address, "gov".to_string());
            assert_eq!(message_amount, coins(fee_amount.u128(), denom));
        }
        _ => panic!("Unexpected return message: {:?}", res.messages[0]),
    }

    // check new channel state after reducing balance
    let chan = query_channel(deps.as_ref(), local_channel.into()).unwrap();
    assert_eq!(
        chan.balances,
        vec![Amount::native(
            fee_amount.u128(),
            &get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
        )]
    );
    assert_eq!(
        chan.total_sent,
        vec![Amount::native(
            amount,
            &get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
        )]
    );

    // mapping pair error with wrong voucher denom
    let pair = UpdatePairMsg {
        local_channel_id: "not_registered_channel".to_string(),
        denom: denom.to_string(),
        local_asset_info: AssetInfo::Token {
            contract_addr: Addr::unchecked("random_cw20_denom".to_string()),
        },
        remote_decimals: 18u8,
        local_asset_info_decimals: 18u8,
    };

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("gov", &[]),
        ExecuteMsg::UpdateMappingPair(pair),
    )
    .unwrap();

    transfer.local_channel_id = "not_registered_channel".to_string();
    let invalid_msg = ExecuteMsg::TransferToRemote(transfer);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), invalid_msg).unwrap_err();
    assert_eq!(err, ContractError::MappingPairNotFound {});
}

#[test]
fn test_update_config() {
    // arrange
    let mut deps = setup(&[], &[]);
    let new_config = ExecuteMsg::UpdateConfig {
        admin: Some("helloworld".to_string()),
        default_timeout: Some(1),
        default_gas_limit: None,
        fee_denom: Some("hehe".to_string()),
        swap_router_contract: Some("new_router".to_string()),
        token_fee: Some(vec![
            TokenFee {
                token_denom: "orai".to_string(),
                ratio: Ratio {
                    nominator: 1,
                    denominator: 10,
                },
            },
            TokenFee {
                token_denom: "atom".to_string(),
                ratio: Ratio {
                    nominator: 1,
                    denominator: 5,
                },
            },
        ]),
        relayer_fee: Some(vec![RelayerFee {
            prefix: "foo".to_string(),
            fee: Uint128::from(1000000u64),
        }]),
        fee_receiver: Some("token_fee_receiver".to_string()),
        relayer_fee_receiver: Some("relayer_fee_receiver".to_string()),
        converter_contract: Some("new_converter".to_string()),
    };
    // unauthorized case
    let unauthorized_info = mock_info(&String::from("somebody"), &[]);
    let is_err = execute(
        deps.as_mut(),
        mock_env(),
        unauthorized_info,
        new_config.clone(),
    )
    .is_err();
    assert_eq!(is_err, true);
    // valid case
    let info = mock_info(&String::from("gov"), &[]);
    execute(deps.as_mut(), mock_env(), info, new_config).unwrap();
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(config.default_gas_limit, None);
    assert_eq!(config.default_timeout, 1);
    assert_eq!(config.fee_denom, "hehe".to_string());
    assert_eq!(config.swap_router_contract, "new_router".to_string());
    assert_eq!(
        config.relayer_fee_receiver,
        Addr::unchecked("relayer_fee_receiver")
    );
    assert_eq!(
        config.token_fee_receiver,
        Addr::unchecked("token_fee_receiver")
    );
    assert_eq!(config.token_fees.len(), 2usize);
    assert_eq!(config.token_fees[0].ratio.denominator, 5);
    assert_eq!(config.token_fees[0].token_denom, "atom".to_string());
    assert_eq!(config.token_fees[1].ratio.denominator, 10);
    assert_eq!(config.token_fees[1].token_denom, "orai".to_string());
    assert_eq!(config.relayer_fees.len(), 1);
    assert_eq!(config.relayer_fees[0].prefix, "foo".to_string());
    assert_eq!(config.relayer_fees[0].amount, Uint128::from(1000000u64));
}

#[test]
fn test_asset_info() {
    let asset_info = AssetInfo::NativeToken {
        denom: "orai".to_string(),
    };
    assert_eq!(asset_info.to_string(), "orai".to_string());
    let asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("oraiaxbc".to_string()),
    };
    assert_eq!(asset_info.to_string(), "oraiaxbc".to_string())
}

#[test]
fn test_handle_packet_refund() {
    let local_channel_id = "channel-0";
    let mut deps = setup(&[local_channel_id], &[]);
    let native_denom = "cosmos";
    let amount = Uint128::from(100u128);
    let sender = "sender";
    let local_asset_info = AssetInfo::NativeToken {
        denom: "orai".to_string(),
    };
    let mapping_denom = format!("wasm.cosmos2contract/{}/{}", local_channel_id, native_denom);

    let result =
        handle_packet_refund(deps.as_mut().storage, sender, native_denom, amount).unwrap_err();
    assert_eq!(
        result.to_string(),
        "cw_ics20::state::MappingMetadata not found"
    );

    // update mapping pair so that we can get refunded
    // cosmos based case with mapping found. Should be successful & cosmos msg is ibc send packet
    // add a pair mapping so we can test the happy case evm based happy case
    let update = UpdatePairMsg {
        local_channel_id: local_channel_id.to_string(),
        denom: native_denom.to_string(),
        local_asset_info: local_asset_info.clone(),
        remote_decimals: 6,
        local_asset_info_decimals: 6,
    };

    let msg = ExecuteMsg::UpdateMappingPair(update.clone());

    let info = mock_info("gov", &coins(1234567, "ucosm"));
    execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

    // now we handle packet failure. should get sub msg
    let result =
        handle_packet_refund(deps.as_mut().storage, sender, &mapping_denom, amount).unwrap();
    assert_eq!(
        result,
        SubMsg::reply_on_error(
            CosmosMsg::Bank(BankMsg::Send {
                to_address: sender.to_string(),
                amount: coins(amount.u128(), "orai")
            }),
            REFUND_FAILURE_ID
        )
    );
}

#[test]
fn test_increase_channel_balance_ibc_receive() {
    let local_channel_id = "channel-0";
    let amount = Uint128::from(10u128);
    let ibc_denom = "foobar";
    let local_receiver = "receiver";
    let mut deps = setup(&[local_channel_id], &[]);

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("attacker", &vec![]),
            ExecuteMsg::IncreaseChannelBalanceIbcReceive {
                dest_channel_id: local_channel_id.to_string(),
                ibc_denom: ibc_denom.to_string(),
                amount: amount.clone(),
                local_receiver: local_receiver.to_string(),
            },
        )
        .unwrap_err(),
        ContractError::Std(StdError::generic_err("Caller is not the contract itself!"))
    );

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(mock_env().contract.address.as_str(), &vec![]),
        ExecuteMsg::IncreaseChannelBalanceIbcReceive {
            dest_channel_id: local_channel_id.to_string(),
            ibc_denom: ibc_denom.to_string(),
            amount: amount.clone(),
            local_receiver: local_receiver.to_string(),
        },
    )
    .unwrap();
    let channel_state = CHANNEL_REVERSE_STATE
        .load(deps.as_ref().storage, (local_channel_id, ibc_denom))
        .unwrap();
    assert_eq!(channel_state.outstanding, amount.clone());
    assert_eq!(channel_state.total_sent, amount.clone());
    let reply_args = REPLY_ARGS.load(deps.as_ref().storage).unwrap();
    assert_eq!(reply_args.amount, amount.clone());
    assert_eq!(reply_args.channel, local_channel_id);
    assert_eq!(reply_args.denom, ibc_denom.to_string());
    assert_eq!(reply_args.local_receiver, local_receiver.to_string());
}

#[test]
fn test_reduce_channel_balance_ibc_receive() {
    let local_channel_id = "channel-0";
    let amount = Uint128::from(10u128);
    let ibc_denom = "foobar";
    let local_receiver = "receiver";
    let mut deps = setup(&[local_channel_id], &[]);
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(mock_env().contract.address.as_str(), &vec![]),
        ExecuteMsg::IncreaseChannelBalanceIbcReceive {
            dest_channel_id: local_channel_id.to_string(),
            ibc_denom: ibc_denom.to_string(),
            amount: amount.clone(),
            local_receiver: local_receiver.to_string(),
        },
    )
    .unwrap();

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("attacker", &vec![]),
            ExecuteMsg::ReduceChannelBalanceIbcReceive {
                src_channel_id: local_channel_id.to_string(),
                ibc_denom: ibc_denom.to_string(),
                amount: amount.clone(),
                local_receiver: local_receiver.to_string(),
            },
        )
        .unwrap_err(),
        ContractError::Std(StdError::generic_err("Caller is not the contract itself!"))
    );

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info(mock_env().contract.address.as_str(), &vec![]),
        ExecuteMsg::ReduceChannelBalanceIbcReceive {
            src_channel_id: local_channel_id.to_string(),
            ibc_denom: ibc_denom.to_string(),
            amount: amount.clone(),
            local_receiver: local_receiver.to_string(),
        },
    )
    .unwrap();
    let channel_state = CHANNEL_REVERSE_STATE
        .load(deps.as_ref().storage, (local_channel_id, ibc_denom))
        .unwrap();
    assert_eq!(channel_state.outstanding, Uint128::zero());
    assert_eq!(channel_state.total_sent, Uint128::from(10u128));
    let reply_args = REPLY_ARGS.load(deps.as_ref().storage).unwrap();
    assert_eq!(reply_args.amount, amount.clone());
    assert_eq!(reply_args.channel, local_channel_id);
    assert_eq!(reply_args.denom, ibc_denom.to_string());
    assert_eq!(reply_args.local_receiver, local_receiver.to_string());
}

#[test]
fn test_query_channel_balance_with_key() {
    // fixture
    let channel = "foo-channel";
    let ibc_denom = "port/channel/denom";
    let amount = Uint128::from(10u128);
    let reduce_amount = Uint128::from(1u128);
    let mut deps = setup(&[channel], &[]);
    increase_channel_balance(deps.as_mut().storage, channel, ibc_denom, amount).unwrap();
    reduce_channel_balance(
        deps.as_mut().storage,
        channel,
        ibc_denom,
        Uint128::from(1u128),
    )
    .unwrap();

    let result =
        query_channel_with_key(deps.as_ref(), channel.to_string(), ibc_denom.to_string()).unwrap();
    assert_eq!(
        result.balance,
        Amount::from_parts(
            ibc_denom.to_string(),
            amount.checked_sub(reduce_amount).unwrap()
        )
    );
    assert_eq!(
        result.total_sent,
        Amount::from_parts(ibc_denom.to_string(), amount)
    );
}

#[test]
fn test_handle_override_channel_balance() {
    // fixture
    let channel = "foo-channel";
    let ibc_denom = "port/channel/denom";
    let amount = Uint128::from(10u128);
    let override_amount = Uint128::from(100u128);
    let total_sent_override = Uint128::from(1000u128);
    let mut deps = setup(&[channel], &[]);
    increase_channel_balance(deps.as_mut().storage, channel, ibc_denom, amount).unwrap();

    // unauthorized case
    let unauthorized = handle_override_channel_balance(
        deps.as_mut(),
        mock_info("attacker", &vec![]),
        channel.to_string(),
        ibc_denom.to_string(),
        amount,
        None,
    )
    .unwrap_err();
    assert_eq!(unauthorized, ContractError::Admin(AdminError::NotAdmin {}));

    // execution, valid case
    handle_override_channel_balance(
        deps.as_mut(),
        mock_info("gov", &vec![]),
        channel.to_string(),
        ibc_denom.to_string(),
        override_amount,
        Some(total_sent_override),
    )
    .unwrap();

    // we query to validate the result after overriding

    let result =
        query_channel_with_key(deps.as_ref(), channel.to_string(), ibc_denom.to_string()).unwrap();
    assert_eq!(
        result.balance,
        Amount::from_parts(ibc_denom.to_string(), override_amount)
    );
    assert_eq!(
        result.total_sent,
        Amount::from_parts(ibc_denom.to_string(), total_sent_override)
    );
}
