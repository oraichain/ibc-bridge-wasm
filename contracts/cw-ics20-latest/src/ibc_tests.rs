#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::{coin, Addr, CosmosMsg, StdError};
    use cw20_ics20_msg::receiver::DestinationInfo;
    use oraiswap::asset::AssetInfo;

    use crate::ibc::{
        build_ibc_msg, check_gas_limit, ibc_packet_receive, parse_swap_to, parse_voucher_denom,
        Ics20Ack, Ics20Packet, RECEIVE_ID,
    };
    use crate::ibc::{build_swap_operations, get_follow_up_msgs};
    use crate::test_helpers::*;
    use cosmwasm_std::{
        from_binary, to_binary, BankMsg, IbcEndpoint, IbcMsg, IbcPacket, IbcPacketReceiveMsg,
        IbcTimeout, SubMsg, Timestamp, Uint128, WasmMsg,
    };

    use crate::error::ContractError;
    use crate::state::{get_key_ics20_ibc_denom, increase_channel_balance};
    use cw20::{Cw20Coin, Cw20ExecuteMsg};
    use cw20_ics20_msg::amount::Amount;

    use crate::contract::{execute, migrate, query_channel};
    use crate::msg::{ExecuteMsg, MigrateMsg, TransferMsg, UpdatePairMsg};
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coins, to_vec, Decimal};
    use cw20::Cw20ReceiveMsg;

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

    fn cw20_payment(
        amount: u128,
        address: &str,
        recipient: &str,
        gas_limit: Option<u64>,
    ) -> SubMsg {
        let msg = Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount: Uint128::new(amount),
        };
        let exec = WasmMsg::Execute {
            contract_addr: address.into(),
            msg: to_binary(&msg).unwrap(),
            funds: vec![],
        };
        let mut msg = SubMsg::reply_on_error(exec, RECEIVE_ID);
        msg.gas_limit = gas_limit;
        msg
    }

    fn native_payment(amount: u128, denom: &str, recipient: &str) -> SubMsg {
        SubMsg::reply_on_error(
            BankMsg::Send {
                to_address: recipient.into(),
                amount: coins(amount, denom),
            },
            RECEIVE_ID,
        )
    }

    fn mock_receive_packet(
        my_channel: &str,
        amount: u128,
        denom: &str,
        receiver: &str,
    ) -> IbcPacket {
        let data = Ics20Packet {
            // this is returning a foreign (our) token, thus denom is <port>/<channel>/<denom>
            denom: format!("{}/{}/{}", REMOTE_PORT, "channel-1234", denom),
            amount: amount.into(),
            sender: "remote-sender".to_string(),
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
    fn send_receive_cw20() {
        let send_channel = "channel-9";
        let cw20_addr = "token-addr";
        let cw20_denom = "cw20:token-addr";
        let gas_limit = 1234567;
        let mut deps = setup(
            &["channel-1", "channel-7", send_channel],
            &[(cw20_addr, gas_limit)],
        );

        // prepare some mock packets
        let recv_packet = mock_receive_packet(send_channel, 876543210, cw20_denom, "local-rcpt");
        let recv_high_packet =
            mock_receive_packet(send_channel, 1876543210, cw20_denom, "local-rcpt");

        // cannot receive this denom yet
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        let no_funds = Ics20Ack::Error(
            ContractError::NoSuchChannelState {
                id: send_channel.to_string(),
                denom: cw20_denom.to_string(),
            }
            .to_string(),
        );
        assert_eq!(ack, no_funds);

        // we send some cw20 tokens over
        let transfer = TransferMsg {
            channel: send_channel.to_string(),
            remote_address: "remote-rcpt".to_string(),
            timeout: None,
            memo: None,
        };
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "local-sender".to_string(),
            amount: Uint128::new(987654321),
            msg: to_binary(&transfer).unwrap(),
        });
        let info = mock_info(cw20_addr, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());
        let expected = Ics20Packet {
            denom: cw20_denom.into(),
            amount: Uint128::new(987654321),
            sender: "local-sender".to_string(),
            receiver: "remote-rcpt".to_string(),
            memo: None,
        };
        let timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
        assert_eq!(
            &res.messages[0],
            &SubMsg::new(IbcMsg::SendPacket {
                channel_id: send_channel.to_string(),
                data: to_binary(&expected).unwrap(),
                timeout: IbcTimeout::with_timestamp(timeout),
            })
        );

        // query channel state|_|
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::cw20(987654321, cw20_addr)]);
        assert_eq!(state.total_sent, vec![Amount::cw20(987654321, cw20_addr)]);

        // cannot receive more than we sent
        let msg = IbcPacketReceiveMsg::new(recv_high_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert_eq!(
            ack,
            Ics20Ack::Error(
                ContractError::InsufficientFunds {
                    id: send_channel.to_string(),
                    denom: cw20_denom.to_string(),
                }
                .to_string(),
            )
        );

        // we can receive less than we sent
        let msg = IbcPacketReceiveMsg::new(recv_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert_eq!(1, res.messages.len());
        assert_eq!(
            cw20_payment(876543210, cw20_addr, "local-rcpt", Some(gas_limit)),
            res.messages[0]
        );
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert!(matches!(ack, Ics20Ack::Result(_)));

        // query channel state
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::cw20(111111111, cw20_addr)]);
        assert_eq!(state.total_sent, vec![Amount::cw20(987654321, cw20_addr)]);
    }

    #[test]
    fn send_receive_native() {
        let send_channel = "channel-9";
        let mut deps = setup(&["channel-1", "channel-7", send_channel], &[]);

        let denom = "uatom";

        // prepare some mock packets
        let recv_packet = mock_receive_packet(send_channel, 876543210, denom, "local-rcpt");
        let recv_high_packet = mock_receive_packet(send_channel, 1876543210, denom, "local-rcpt");

        // cannot receive this denom yet
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        let no_funds = Ics20Ack::Error(
            ContractError::InsufficientFunds {
                id: send_channel.to_string(),
                denom: denom.to_string(),
            }
            .to_string(),
        );
        assert_eq!(ack, no_funds);

        // we transfer some tokens
        let msg = ExecuteMsg::Transfer(TransferMsg {
            channel: send_channel.to_string(),
            remote_address: "my-remote-address".to_string(),
            timeout: None,
            memo: Some("memo".to_string()),
        });
        let info = mock_info("local-sender", &coins(987654321, denom));
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // query channel state|_|
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::native(987654321, denom)]);
        assert_eq!(state.total_sent, vec![Amount::native(987654321, denom)]);

        // cannot receive more than we sent
        let msg = IbcPacketReceiveMsg::new(recv_high_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert_eq!(ack, no_funds);

        // we can receive less than we sent
        let msg = IbcPacketReceiveMsg::new(recv_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert_eq!(1, res.messages.len());
        assert_eq!(
            native_payment(876543210, denom, "local-rcpt"),
            res.messages[0]
        );
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert!(matches!(ack, Ics20Ack::Result(_)));

        // only need to call reply block on error case

        // query channel state
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::native(111111111, denom)]);
        assert_eq!(state.total_sent, vec![Amount::native(987654321, denom)]);
    }

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
    ) -> IbcPacket {
        let data = Ics20Packet {
            // this is returning a foreign native token, thus denom is <denom>, eg: uatom
            denom: format!("{}", denom),
            amount: amount.into(),
            sender: "remote-sender".to_string(),
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
        let recv_packet =
            mock_receive_packet_remote_to_local(send_channel, 876543210, cw20_denom, custom_addr);

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
    fn send_native_from_remote_receive_happy_path() {
        let send_channel = "channel-9";
        let cw20_addr = "token-addr";
        let custom_addr = "custom-addr";
        let denom = "uatom";
        let asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked("cw20:token-addr"),
        };
        let gas_limit = 1234567;
        let mut deps = setup(
            &["channel-1", "channel-7", send_channel],
            &[(cw20_addr, gas_limit)],
        );

        let pair = UpdatePairMsg {
            local_channel_id: send_channel.to_string(),
            denom: denom.to_string(),
            asset_info: asset_info,
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
        let recv_packet =
            mock_receive_packet_remote_to_local(send_channel, 876543210, denom, custom_addr);

        // we can receive this denom, channel balance should increase
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        println!("res: {:?}", res);
        assert_eq!(res.messages.len(), 1);
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        println!("ack: {:?}", ack);
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
            contract_addr: Addr::unchecked("foobar"),
        };
        let cw20_address = Addr::unchecked("addr");
        let fee_denom = "orai".to_string();
        let destination: DestinationInfo = DestinationInfo {
            receiver: "cosmos".to_string(),
            destination_channel: "channel-1".to_string(),
            destination_denom: "foobar".to_string(),
        };

        let operations = build_swap_operations(
            receiver_asset_info.clone(),
            cw20_address.clone(),
            fee_denom.as_str(),
            &destination,
        );
        assert_eq!(operations.len(), 2);

        let fee_denom = "foobar".to_string();
        let operations =
            build_swap_operations(receiver_asset_info, cw20_address, &fee_denom, &destination);
        assert_eq!(operations.len(), 1);
    }

    #[test]
    fn test_get_ibc_msg() {
        let send_channel = "channel-9";
        let receive_channel = "channel-1";
        let allowed = "foobar";
        let allowed_gas = 777666;
        let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);
        let receiver_asset_info = AssetInfo::NativeToken {
            denom: "orai".to_string(),
        };
        let amount = Uint128::from(10u128);
        let remote_address = "eth-mainnet0x1235";
        let ibc_wasm_addr = "addr";
        let mut destination = DestinationInfo {
            receiver: "0x1234".to_string(),
            destination_channel: "channel-10".to_string(),
            destination_denom: "atom".to_string(),
        };
        let timeout = 1000u64;

        // first case, destination channel empty
        destination.destination_channel = "".to_string();

        let err = build_ibc_msg(
            deps.as_mut().storage,
            &receiver_asset_info.to_string(),
            receive_channel,
            amount,
            remote_address,
            ibc_wasm_addr,
            &destination,
            timeout,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("Destination channel empty"));

        // not evm based case, should be successful & cosmos msg is ibc transfer
        destination.destination_channel = "channel-10".to_string();
        let result = build_ibc_msg(
            deps.as_mut().storage,
            &receiver_asset_info.to_string(),
            receive_channel,
            amount,
            remote_address,
            ibc_wasm_addr,
            &destination,
            timeout,
        )
        .unwrap();
        assert_eq!(
            result,
            CosmosMsg::Ibc(IbcMsg::Transfer {
                channel_id: "channel-10".to_string(),
                to_address: "0x1234".to_string(),
                amount: coin(10u128, "atom"),
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(1000u64))
            })
        );

        // evm based case, error getting pair mapping
        destination.receiver = "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string();
        let err = build_ibc_msg(
            deps.as_mut().storage,
            &receiver_asset_info.to_string(),
            receive_channel,
            amount,
            remote_address,
            ibc_wasm_addr,
            &destination,
            timeout,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("cannot find pair mappings"));

        // add a pair mapping so we can test the happy case
        let update = UpdatePairMsg {
            local_channel_id: "mars-channel".to_string(),
            denom: "trx-mainnet".to_string(),
            asset_info: receiver_asset_info.clone(),
            remote_decimals: 18,
            asset_info_decimals: 18,
        };

        // works with proper funds
        let msg = ExecuteMsg::UpdateMappingPair(update.clone());

        let info = mock_info("gov", &coins(1234567, "ucosm"));
        execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
        let pair_mapping_key = format!(
            "wasm.{}/{}/{}",
            "cosmos2contract", update.local_channel_id, "trx-mainnet"
        );
        increase_channel_balance(
            deps.as_mut().storage,
            receive_channel,
            pair_mapping_key.as_str(),
            amount.clone(),
            false,
        )
        .unwrap();
        destination.receiver = "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string();
        destination.destination_channel = "trx-mainnet".to_string();
        let result = build_ibc_msg(
            deps.as_mut().storage,
            &receiver_asset_info.to_string(),
            receive_channel,
            amount,
            remote_address,
            ibc_wasm_addr,
            &destination,
            timeout,
        )
        .unwrap();

        assert_eq!(
            result,
            CosmosMsg::Ibc(IbcMsg::SendPacket {
                channel_id: receive_channel.to_string(),
                data: to_binary(&Ics20Packet::new(
                    amount.clone(),
                    pair_mapping_key,
                    ibc_wasm_addr,
                    &remote_address,
                    Some(destination.receiver),
                ))
                .unwrap(),
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(1000u64))
            })
        );
    }

    #[test]
    fn test_follow_up_msgs() {
        let send_channel = "channel-9";
        let allowed = "foobar";
        let allowed_gas = 777666;
        let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);
        let deps_mut = deps.as_mut();
        let receiver = "foobar";
        let amount = Uint128::from(1u128);
        let ibc_wasm_addr = Addr::unchecked("foobar");

        // first case, memo empty => return send amount with receiver input
        let result = get_follow_up_msgs(
            deps_mut.storage,
            deps_mut.api,
            &deps_mut.querier,
            ibc_wasm_addr.clone(),
            Amount::Cw20(Cw20Coin {
                address: "foobar".to_string(),
                amount: amount.clone(),
            }),
            "foobar",
            receiver.clone(),
            "",
            &mock_receive_packet_remote_to_local("channel", 1u128, "foobar", "foobar"),
        )
        .unwrap();

        assert_eq!(
            result,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: ibc_wasm_addr.clone().into_string(),
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
            ibc_wasm_addr.clone(),
            Amount::Cw20(Cw20Coin {
                address: "foobar".to_string(),
                amount,
            }),
            "foobar",
            "foobar",
            memo,
            &mock_receive_packet_remote_to_local("channel", 1u128, "foobar", "foobar"),
        )
        .unwrap();

        assert_eq!(
            result,
            vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: ibc_wasm_addr.clone().into_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: "cosmosabcd".to_string(),
                    amount: amount.clone()
                })
                .unwrap(),
                funds: vec![]
            })]
        );
    }

    #[test]
    pub fn test_parse_dest_receiver() {
        assert_eq!(parse_swap_to("abcd", ""), None);
        assert_eq!(parse_swap_to("abcd", "abcd"), None);
        assert_eq!(parse_swap_to("", ""), Some("".to_string()));
        assert_eq!(parse_swap_to("", "abcd"), Some("abcd".to_string()))
    }
}
