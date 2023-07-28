#[cfg(test)]
mod test {
    use cosmwasm_std::{attr, coin, Addr, CosmosMsg, IbcTimeout, Response, StdError};
    use cw20_ics20_msg::receiver::DestinationInfo;
    use oraiswap::asset::AssetInfo;
    use oraiswap::router::SwapOperation;

    use crate::ibc::{
        ack_fail, build_ibc_msg, build_swap_msgs, check_gas_limit,
        convert_remote_denom_to_evm_prefix, deduct_fee, deduct_relayer_fee, deduct_token_fee,
        handle_follow_up_failure, ibc_packet_receive, is_follow_up_msgs_only_send_amount,
        parse_voucher_denom, parse_voucher_denom_without_sanity_checks, process_ibc_msg, Ics20Ack,
        Ics20Packet, REFUND_FAILURE_ID,
    };
    use crate::ibc::{build_swap_operations, get_follow_up_msgs};
    use crate::test_helpers::*;
    use cosmwasm_std::{
        from_binary, to_binary, IbcEndpoint, IbcMsg, IbcPacket, IbcPacketReceiveMsg, SubMsg,
        Timestamp, Uint128, WasmMsg,
    };

    use crate::error::ContractError;
    use crate::state::{
        get_key_ics20_ibc_denom, increase_channel_balance, ChannelState, IbcSingleStepData,
        MappingMetadata, Ratio, SingleStepReplyArgs, CHANNEL_REVERSE_STATE, RELAYER_FEE,
        SINGLE_STEP_REPLY_ARGS, TOKEN_FEE, TOKEN_FEE_ACCUMULATOR,
    };
    use cw20::{Cw20Coin, Cw20ExecuteMsg};
    use cw20_ics20_msg::amount::{convert_local_to_remote, Amount};

    use crate::contract::{execute, migrate, query_channel};
    use crate::msg::{ExecuteMsg, MigrateMsg, UpdatePairMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, to_vec};

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

    // fn cw20_payment(
    //     amount: u128,
    //     address: &str,
    //     recipient: &str,
    //     gas_limit: Option<u64>,
    // ) -> SubMsg {
    //     let msg = Cw20ExecuteMsg::Transfer {
    //         recipient: recipient.into(),
    //         amount: Uint128::new(amount),
    //     };
    //     let exec = WasmMsg::Execute {
    //         contract_addr: address.into(),
    //         msg: to_binary(&msg).unwrap(),
    //         funds: vec![],
    //     };
    //     let mut msg = SubMsg::reply_on_error(exec, RECEIVE_ID);
    //     msg.gas_limit = gas_limit;
    //     msg
    // }

    // fn _native_payment(amount: u128, denom: &str, recipient: &str) -> SubMsg {
    //     SubMsg::reply_on_error(
    //         BankMsg::Send {
    //             to_address: recipient.into(),
    //             amount: coins(amount, denom),
    //         },
    //         RECEIVE_ID,
    //     )
    // }

    // fn mock_receive_packet(
    //     my_channel: &str,
    //     amount: u128,
    //     denom: &str,
    //     receiver: &str,
    // ) -> IbcPacket {
    //     let data = Ics20Packet {
    //         // this is returning a foreign (our) token, thus denom is <port>/<channel>/<denom>
    //         denom: format!("{}/{}/{}", REMOTE_PORT, "channel-1234", denom),
    //         amount: amount.into(),
    //         sender: "remote-sender".to_string(),
    //         receiver: receiver.to_string(),
    //         memo: None,
    //     };
    //     IbcPacket::new(
    //         to_binary(&data).unwrap(),
    //         IbcEndpoint {
    //             port_id: REMOTE_PORT.to_string(),
    //             channel_id: "channel-1234".to_string(),
    //         },
    //         IbcEndpoint {
    //             port_id: CONTRACT_PORT.to_string(),
    //             channel_id: my_channel.to_string(),
    //         },
    //         3,
    //         Timestamp::from_seconds(1665321069).into(),
    //     )
    // }

    // #[test]
    // fn send_receive_cw20() {
    //     let send_channel = "channel-9";
    //     let cw20_addr = "token-addr";
    //     let cw20_denom = "cw20:token-addr";
    //     let gas_limit = 1234567;
    //     let mut deps = setup(
    //         &["channel-1", "channel-7", send_channel],
    //         &[(cw20_addr, gas_limit)],
    //     );

    //     // prepare some mock packets
    //     let recv_packet = mock_receive_packet(send_channel, 876543210, cw20_denom, "local-rcpt");
    //     let recv_high_packet =
    //         mock_receive_packet(send_channel, 1876543210, cw20_denom, "local-rcpt");

    //     // cannot receive this denom yet
    //     let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
    //     let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
    //     assert!(res.messages.is_empty());
    //     let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
    //     let no_funds = Ics20Ack::Error(
    //         ContractError::NoSuchChannelState {
    //             id: send_channel.to_string(),
    //             denom: cw20_denom.to_string(),
    //         }
    //         .to_string(),
    //     );
    //     assert_eq!(ack, no_funds);

    //     // we send some cw20 tokens over
    //     let transfer = TransferMsg {
    //         channel: send_channel.to_string(),
    //         remote_address: "remote-rcpt".to_string(),
    //         timeout: None,
    //         memo: None,
    //     };
    //     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
    //         sender: "local-sender".to_string(),
    //         amount: Uint128::new(987654321),
    //         msg: to_binary(&transfer).unwrap(),
    //     });
    //     let info = mock_info(cw20_addr, &[]);
    //     let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    //     assert_eq!(1, res.messages.len());
    //     let expected = Ics20Packet {
    //         denom: cw20_denom.into(),
    //         amount: Uint128::new(987654321),
    //         sender: "local-sender".to_string(),
    //         receiver: "remote-rcpt".to_string(),
    //         memo: None,
    //     };
    //     let timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
    //     assert_eq!(
    //         &res.messages[0],
    //         &SubMsg::new(IbcMsg::SendPacket {
    //             channel_id: send_channel.to_string(),
    //             data: to_binary(&expected).unwrap(),
    //             timeout: IbcTimeout::with_timestamp(timeout),
    //         })
    //     );

    //     // query channel state|_|
    //     let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
    //     assert_eq!(state.balances, vec![Amount::cw20(987654321, cw20_addr)]);
    //     assert_eq!(state.total_sent, vec![Amount::cw20(987654321, cw20_addr)]);

    //     // cannot receive more than we sent
    //     let msg = IbcPacketReceiveMsg::new(recv_high_packet);
    //     let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
    //     assert!(res.messages.is_empty());
    //     let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
    //     assert_eq!(
    //         ack,
    //         Ics20Ack::Error(
    //             ContractError::InsufficientFunds {
    //                 id: send_channel.to_string(),
    //                 denom: cw20_denom.to_string(),
    //             }
    //             .to_string(),
    //         )
    //     );

    //     // we can receive less than we sent
    //     let msg = IbcPacketReceiveMsg::new(recv_packet);
    //     let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
    //     assert_eq!(1, res.messages.len());
    //     assert_eq!(
    //         cw20_payment(876543210, cw20_addr, "local-rcpt", Some(gas_limit)),
    //         res.messages[0]
    //     );
    //     let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
    //     assert!(matches!(ack, Ics20Ack::Result(_)));

    //     // query channel state
    //     let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
    //     assert_eq!(state.balances, vec![Amount::cw20(111111111, cw20_addr)]);
    //     assert_eq!(state.total_sent, vec![Amount::cw20(987654321, cw20_addr)]);
    // }

    // #[test]
    // fn send_receive_native() {
    //     let send_channel = "channel-9";
    //     let mut deps = setup(&["channel-1", "channel-7", send_channel], &[]);

    //     let denom = "uatom";

    //     // prepare some mock packets
    //     let recv_packet = mock_receive_packet(send_channel, 876543210, denom, "local-rcpt");
    //     let recv_high_packet = mock_receive_packet(send_channel, 1876543210, denom, "local-rcpt");

    //     // cannot receive this denom yet
    //     let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
    //     let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
    //     assert!(res.messages.is_empty());
    //     let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
    //     let no_funds = Ics20Ack::Error(
    //         ContractError::NoSuchChannelState {
    //             id: send_channel.to_string(),
    //             denom: denom.to_string(),
    //         }
    //         .to_string(),
    //     );
    //     assert_eq!(ack, no_funds);

    //     // we transfer some tokens
    //     let msg = ExecuteMsg::Transfer(TransferMsg {
    //         channel: send_channel.to_string(),
    //         remote_address: "my-remote-address".to_string(),
    //         timeout: None,
    //         memo: Some("memo".to_string()),
    //     });
    //     let info = mock_info("local-sender", &coins(987654321, denom));
    //     execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    //     // query channel state|_|
    //     let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
    //     assert_eq!(state.balances, vec![Amount::native(987654321, denom)]);
    //     assert_eq!(state.total_sent, vec![Amount::native(987654321, denom)]);

    //     // cannot receive more than we sent
    //     let msg = IbcPacketReceiveMsg::new(recv_high_packet);
    //     let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
    //     assert!(res.messages.is_empty());
    //     let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
    //     assert_eq!(
    //         ack,
    //         Ics20Ack::Error(
    //             ContractError::InsufficientFunds {
    //                 id: send_channel.to_string(),
    //                 denom: denom.to_string(),
    //             }
    //             .to_string(),
    //         )
    //     );

    //     // we can receive less than we sent
    //     let msg = IbcPacketReceiveMsg::new(recv_packet);
    //     let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
    //     assert_eq!(1, res.messages.len());
    //     assert_eq!(
    //         native_payment(876543210, denom, "local-rcpt"),
    //         res.messages[0]
    //     );
    //     let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
    //     assert!(matches!(ack, Ics20Ack::Result(_)));

    //     // only need to call reply block on error case

    //     // query channel state
    //     let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
    //     assert_eq!(state.balances, vec![Amount::native(111111111, denom)]);
    //     assert_eq!(state.total_sent, vec![Amount::native(987654321, denom)]);
    // }

    #[test]
    fn check_gas_limit_handles_all_cases() {
        let send_channel = "channel-9";
        let allowed = "foobar";
        let allowed_gas = 777666;
        let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);

        // allow list will get proper gas
        let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, allowed)).unwrap();
        assert_eq!(limit, Some(allowed_gas));

        // non-allow list will error
        let random = "tokenz";
        check_gas_limit(deps.as_ref(), &Amount::cw20(500, random)).unwrap_err();

        // add default_gas_limit
        let def_limit = 54321;
        migrate(
            deps.as_mut(),
            mock_env(),
            MigrateMsg {
                default_gas_limit: Some(def_limit),
                fee_receiver: "receiver".to_string(),
                default_timeout: 100u64,
                fee_denom: "orai".to_string(),
                swap_router_contract: "foobar".to_string(),
            },
        )
        .unwrap();

        // allow list still gets proper gas
        let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, allowed)).unwrap();
        assert_eq!(limit, Some(allowed_gas));

        // non-allow list will now get default
        let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, random)).unwrap();
        assert_eq!(limit, Some(def_limit));
    }

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
        let recv_packet = mock_receive_packet_remote_to_local(
            send_channel,
            876543210,
            cw20_denom,
            custom_addr,
            None,
        );

        // we can receive this denom, channel balance should increase
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        // assert_eq!(res, StdError)
        assert_eq!(
            res.attributes.last().unwrap().value,
            "You can only send native tokens that has a map to the corresponding asset info"
        );
    }

    #[test]
    fn send_from_remote_to_local_receive_happy_path() {
        let send_channel = "channel-9";
        let cw20_addr = "token-addr";
        let custom_addr = "custom-addr";
        let denom = "uatom0x";
        let asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked(cw20_addr),
        };
        let gas_limit = 1234567;
        let send_amount = Uint128::from(876543210u64);
        let mut deps = setup(
            &["channel-1", "channel-7", send_channel],
            &[(cw20_addr, gas_limit)],
        );
        TOKEN_FEE
            .save(
                deps.as_mut().storage,
                denom,
                &Ratio {
                    numerator: 1,
                    denominator: 10,
                },
            )
            .unwrap();

        let pair = UpdatePairMsg {
            local_channel_id: send_channel.to_string(),
            denom: denom.to_string(),
            asset_info: asset_info.clone(),
            remote_decimals: 18u8,
            asset_info_decimals: 18u8,
        };

        let _ = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("gov", &[]),
            ExecuteMsg::UpdateMappingPair(pair),
        )
        .unwrap();

        // prepare some mock packets
        let recv_packet = mock_receive_packet_remote_to_local(
            send_channel,
            send_amount.u128(),
            denom,
            custom_addr,
            Some("orai1cdhkt9ps47hwn9sqren70uw9cyrfka9fpauuks"),
        );

        // we can receive this denom, channel balance should increase
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        println!("res: {:?}", res);
        // TODO: fix test cases. Possibly because we are adding two add_submessages?
        assert_eq!(res.messages.len(), 2); // 2 messages because we also have deduct fee msg
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
                        recipient: "gov".to_string(),
                        amount: Uint128::from(87654321u64)
                    })
                    .unwrap()
                );
            }
            _ => panic!("Unexpected return message: {:?}", res.messages[0]),
        }
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert!(matches!(ack, Ics20Ack::Result(_)));

        // query channel state|_|
        let state = query_channel(deps.as_ref(), send_channel.to_string(), None).unwrap();
        assert_eq!(
            state.balances,
            vec![Amount::native(
                876543210,
                &get_key_ics20_ibc_denom(CONTRACT_PORT, send_channel, denom)
            )]
        );
        assert_eq!(
            state.total_sent,
            vec![Amount::native(
                876543210,
                &get_key_ics20_ibc_denom(CONTRACT_PORT, send_channel, denom)
            )]
        );
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
        let mut cosmos_msgs: Vec<CosmosMsg> = vec![];
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
            cosmos_msgs[0]
        );
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
            deps.as_mut().storage,
            env.clone(),
            receiver_asset_info.clone(),
            local_receiver,
            receive_channel,
            amount,
            remote_address,
            &destination,
            timeout,
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
            deps.as_mut().storage,
            env.clone(),
            receiver_asset_info.clone(),
            local_receiver,
            receive_channel,
            amount,
            remote_address,
            &destination,
            timeout,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("cannot find pair mappings"));

        // add a pair mapping so we can test the happy case evm based happy case
        let update = UpdatePairMsg {
            local_channel_id: "mars-channel".to_string(),
            denom: pair_mapping_denom.to_string(),
            asset_info: receiver_asset_info.clone(),
            remote_decimals,
            asset_info_decimals,
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
            false,
        )
        .unwrap();
        destination.receiver = "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string();
        destination.destination_channel = "trx-mainnet".to_string();
        let result = build_ibc_msg(
            deps.as_mut().storage,
            env.clone(),
            receiver_asset_info.clone(),
            local_receiver,
            receive_channel,
            amount,
            remote_address,
            &destination,
            timeout,
        )
        .unwrap();

        assert_eq!(
            result,
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
            })
        );
        let reply_args = SINGLE_STEP_REPLY_ARGS.load(deps.as_mut().storage).unwrap();
        let ibc_data = reply_args.ibc_data.unwrap();
        assert_eq!(ibc_data.remote_amount, remote_amount);
        assert_eq!(reply_args.local_amount, amount);
        assert_eq!(reply_args.channel, receive_channel);
        assert_eq!(ibc_data.ibc_denom, pair_mapping_key);
        assert_eq!(reply_args.receiver, local_receiver.to_string());
        assert_eq!(reply_args.refund_asset_info, receiver_asset_info)
    }

    #[test]
    fn test_get_ibc_msg_cosmos_based_case() {
        // setup
        let send_channel = "channel-9";
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
            destination_channel: "channel-10".to_string(),
            destination_denom: "atom".to_string(),
        };
        let env = mock_env();
        let remote_address = "foobar";
        let ibc_denom = format!("foo/bar/{}", pair_mapping_denom);
        let remote_decimals = 18;
        let asset_info_decimals = 6;
        let remote_channel = "mars-channel";
        let pair_mapping_key = format!(
            "wasm.cosmos2contract/{}/{}",
            remote_channel, pair_mapping_denom
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

        // cosmos based case but no mapping found. should be successful & cosmos msg is ibc transfer
        let result = build_ibc_msg(
            deps.as_mut().storage,
            env.clone(),
            receiver_asset_info.clone(),
            local_receiver,
            local_channel_id,
            amount,
            remote_address,
            &destination,
            timeout,
        )
        .unwrap();
        assert_eq!(
            result,
            CosmosMsg::Ibc(IbcMsg::Transfer {
                channel_id: "channel-10".to_string(),
                to_address: destination.receiver.clone(),
                amount: coin(1000u128, "atom"),
                timeout: mock_env().block.time.plus_seconds(timeout).into()
            })
        );

        // cosmos based case with mapping found. Should be successful & cosmos msg is ibc send packet
        // add a pair mapping so we can test the happy case evm based happy case
        let update = UpdatePairMsg {
            local_channel_id: "mars-channel".to_string(),
            denom: pair_mapping_denom.to_string(),
            asset_info: receiver_asset_info.clone(),
            remote_decimals,
            asset_info_decimals,
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
            deps.as_mut().storage,
            env.clone(),
            receiver_asset_info.clone(),
            local_receiver,
            local_channel_id,
            amount,
            remote_address,
            &destination,
            timeout,
        )
        .unwrap();

        assert_eq!(
            result,
            CosmosMsg::Ibc(IbcMsg::SendPacket {
                channel_id: local_channel_id.to_string(),
                data: to_binary(&Ics20Packet::new(
                    remote_amount.clone(),
                    pair_mapping_key.clone(),
                    env.contract.address.as_str(),
                    &destination.receiver,
                    None,
                ))
                .unwrap(),
                timeout: env.block.time.plus_seconds(timeout).into()
            })
        );
    }

    #[test]
    fn test_get_ibc_msg_neither_cosmos_or_evm_based_case() {
        // setup
        let send_channel = "channel-9";
        let allowed = "foobar";
        let allowed_gas = 777666;
        let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);
        let amount = Uint128::from(1000u64);
        let receiver_asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked("usdt"),
        };
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
            deps.as_mut().storage,
            env.clone(),
            receiver_asset_info.clone(),
            local_receiver,
            local_channel_id,
            amount,
            remote_address,
            &destination,
            timeout,
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
            "foobar",
            receiver.clone(),
            "",
            local_channel,
        )
        .unwrap();

        assert_eq!(
            result.0,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: receiver.to_string(),
                    amount: amount.clone()
                })
                .unwrap(),
                funds: vec![]
            })]
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
            "foobar",
            "foobar",
            memo,
            local_channel,
        )
        .unwrap();

        assert_eq!(
            result.0,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: receiver.to_string(),
                    amount: amount.clone()
                })
                .unwrap(),
                funds: vec![]
            })]
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
            "foobar",
            "foobar",
            memo,
            local_channel,
        )
        .unwrap();

        assert_eq!(
            result.0,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: receiver.to_string(),
                    amount: amount.clone()
                })
                .unwrap(),
                funds: vec![]
            })]
        );
    }

    #[test]
    fn test_handle_follow_up_failure() {
        let local_channel_id = "channel-0";
        let mut deps = setup(&[local_channel_id], &[]);
        let native_denom = "cosmos";
        let refund_asset_info = AssetInfo::NativeToken {
            denom: native_denom.to_string(),
        };
        let amount = Uint128::from(100u128);
        let receiver = "receiver";
        let err = "ack_failed";
        let mut single_step_reply_args = SingleStepReplyArgs {
            channel: local_channel_id.to_string(),
            refund_asset_info: refund_asset_info.clone(),
            ibc_data: None,
            local_amount: amount,
            receiver: receiver.to_string(),
        };
        let result = handle_follow_up_failure(
            deps.as_mut().storage,
            single_step_reply_args.clone(),
            err.to_string(),
        )
        .unwrap();
        assert_eq!(
            result,
            Response::new()
                .add_submessage(SubMsg::reply_on_error(
                    Amount::from_parts(native_denom.to_string(), amount.clone())
                        .send_amount(single_step_reply_args.receiver.clone(), None),
                    REFUND_FAILURE_ID
                ))
                .set_data(ack_fail(err.to_string()))
                .add_attributes(vec![
                    attr("error_follow_up_msgs", err),
                    attr(
                        "attempt_refund_denom",
                        single_step_reply_args.refund_asset_info.to_string(),
                    ),
                    attr("attempt_refund_amount", single_step_reply_args.local_amount),
                ])
        );
        let ibc_denom = "ibc_denom";
        let remote_amount = convert_local_to_remote(amount, 18, 6).unwrap();
        single_step_reply_args.ibc_data = Some(IbcSingleStepData {
            ibc_denom: ibc_denom.to_string(),
            remote_amount: remote_amount.clone(),
        });
        // if has ibc denom then it's evm based, need to undo reducing balance
        CHANNEL_REVERSE_STATE
            .save(
                deps.as_mut().storage,
                (local_channel_id, ibc_denom),
                &ChannelState {
                    outstanding: Uint128::from(0u128),
                    total_sent: Uint128::from(100u128),
                },
            )
            .unwrap();
        handle_follow_up_failure(
            deps.as_mut().storage,
            single_step_reply_args.clone(),
            err.to_string(),
        )
        .unwrap();
        let channel_state = CHANNEL_REVERSE_STATE
            .load(deps.as_mut().storage, (local_channel_id, ibc_denom))
            .unwrap();
        // should undo reduce channel state
        assert_eq!(channel_state.outstanding, remote_amount)
    }

    #[test]
    fn test_is_follow_up_msgs_only_send_amount() {
        assert_eq!(is_follow_up_msgs_only_send_amount("", "dest denom"), true);
        assert_eq!(is_follow_up_msgs_only_send_amount("memo", ""), true);
        assert_eq!(
            is_follow_up_msgs_only_send_amount("memo", "dest denom"),
            false
        );
    }

    #[test]
    fn test_deduct_fee() {
        assert_eq!(
            deduct_fee(
                Ratio {
                    numerator: 1,
                    denominator: 0,
                },
                Uint128::from(1000u64)
            ),
            Uint128::from(0u64)
        );
        assert_eq!(
            deduct_fee(
                Ratio {
                    numerator: 1,
                    denominator: 1,
                },
                Uint128::from(1000u64)
            ),
            Uint128::from(1000u64)
        );
        assert_eq!(
            deduct_fee(
                Ratio {
                    numerator: 1,
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
    fn test_parse_voucher_denom_without_sanity_checks() {
        assert_eq!(
            parse_voucher_denom_without_sanity_checks("foo").is_err(),
            true
        );
        assert_eq!(
            parse_voucher_denom_without_sanity_checks("foo/bar").is_err(),
            true
        );
        let result = parse_voucher_denom_without_sanity_checks("foo/bar/helloworld").unwrap();
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn test_deduct_token_fee() {
        let mut deps = mock_dependencies();
        let amount = Uint128::from(1000u64);
        let storage = deps.as_mut().storage;
        let token_fee_denom = "foo0x";
        // should return amount because we have not set relayer fee yet
        assert_eq!(
            deduct_token_fee(storage, "foo", amount, "foo").unwrap().0,
            amount.clone()
        );
        TOKEN_FEE
            .save(
                storage,
                token_fee_denom,
                &Ratio {
                    numerator: 1,
                    denominator: 100,
                },
            )
            .unwrap();
        assert_eq!(
            deduct_token_fee(storage, token_fee_denom, amount, "foo")
                .unwrap()
                .0,
            Uint128::from(990u64)
        );
        assert_eq!(
            TOKEN_FEE_ACCUMULATOR.load(storage, "foo").unwrap(),
            Uint128::from(10u64)
        );
    }

    #[test]
    fn test_deduct_relayer_fee() {
        let mut deps = mock_dependencies();
        let amount = Uint128::from(1000u64);
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
            amount,
            offer_amount.clone(),
            "local_token_denom",
            Uint128::from(0u64),
        )
        .unwrap();
        assert_eq!(result.1, Uint128::from(0u64));

        // remote address is wrong (dont follow bech32 form)
        assert_eq!(
            deduct_relayer_fee(
                deps_mut.storage,
                deps_mut.api,
                "foobar",
                token_fee_denom,
                amount,
                offer_amount.clone(),
                "local_token_denom",
                token_price,
            )
            .unwrap_err(),
            StdError::generic_err("Cannot decode remote sender: foobar")
        );

        // no relayer fee case
        assert_eq!(
            deduct_relayer_fee(
                deps_mut.storage,
                deps_mut.api,
                remote_address,
                token_fee_denom,
                amount,
                offer_amount.clone(),
                "local_token_denom",
                token_price,
            )
            .unwrap()
            .1,
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
                amount,
                offer_amount.clone(),
                "local_token_denom",
                token_price,
            )
            .unwrap()
            .1,
            Uint128::from(100u64)
        );

        // normal case with remote address
        assert_eq!(
            deduct_relayer_fee(
                deps_mut.storage,
                deps_mut.api,
                remote_address,
                token_fee_denom,
                amount,
                offer_amount.clone(),
                "local_token_denom",
                token_price,
            )
            .unwrap()
            .1,
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
        let pair_mapping = (
            ibc_denom.to_string(),
            MappingMetadata {
                asset_info: AssetInfo::NativeToken {
                    denom: "orai".to_string(),
                },
                remote_decimals: 18,
                asset_info_decimals: 6,
            },
        );
        let receiver_asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked("usdt"),
        };
        let local_channel_id = "channel";
        let ibc_msg_sender = "sender";
        let ibc_msg_receiver = "receiver";
        let memo = None;
        let timeout = Timestamp::from_seconds(10u64);
        let reply_args: SingleStepReplyArgs = SingleStepReplyArgs {
            channel: local_channel_id.to_string(),
            refund_asset_info: receiver_asset_info.clone(),
            ibc_data: None,
            local_amount: amount.clone(),
            receiver: ibc_msg_receiver.to_string(),
        };
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
            storage,
            pair_mapping,
            receiver_asset_info,
            local_channel_id,
            ibc_msg_sender,
            ibc_msg_receiver,
            memo,
            amount,
            timeout,
            reply_args,
        )
        .unwrap();

        // assert
        // channel balance should reduce to 0
        assert_eq!(
            CHANNEL_REVERSE_STATE
                .load(storage, (local_channel_id, ibc_denom))
                .unwrap()
                .outstanding,
            Uint128::from(0u64)
        );
        // reply args should have ibc data now
        assert_eq!(
            SINGLE_STEP_REPLY_ARGS
                .load(storage)
                .unwrap()
                .ibc_data
                .unwrap(),
            IbcSingleStepData {
                ibc_denom: ibc_denom.to_string(),
                remote_amount
            }
        );
        assert_eq!(
            result,
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
            }
        );
    }
}
