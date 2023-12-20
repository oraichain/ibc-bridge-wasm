use crate::ibc::Ics20Packet;
use crate::msg::{AllowMsg, InitMsg, UpdatePairMsg};
use crate::test_helpers::{CONTRACT_PORT, DEFAULT_TIMEOUT, REMOTE_PORT, WASM_BYTES};

use cosmwasm_std::{to_binary, Addr, Coin, IbcEndpoint, IbcPacket, Timestamp};
use oraiswap::asset::{AssetInfo, ORAI_DENOM};
use osmosis_test_tube::{Module, OraichainTestApp, Wasm};
use test_tube::Account;

use crate::msg::ExecuteMsg;

fn mock_app() -> OraichainTestApp {
    let router = OraichainTestApp::default();
    router
}

fn _mock_receive_packet(
    my_channel: &str,
    remote_channel: &str,
    amount: u128,
    native_denom: &str,
    remote_sender: &str,
    receiver: &str,
) -> IbcPacket {
    let data = Ics20Packet {
        // this is returning a foreign (our) token, thus denom is <port>/<channel>/<denom>
        denom: format!("{}/{}/{}", REMOTE_PORT, remote_channel, native_denom),
        amount: amount.into(),
        sender: remote_sender.to_string(),
        receiver: receiver.to_string(),
        memo: None,
    };
    IbcPacket::new(
        to_binary(&data).unwrap(),
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: remote_channel.to_string(),
        },
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: my_channel.to_string(),
        },
        3,
        Timestamp::from_seconds(1665321069).into(),
    )
}

fn initialize_basic_data_for_testings() -> (
    OraichainTestApp,
    Addr,
    String,
    IbcEndpoint,
    String,
    String,
    String,
    u8,
) {
    let router = mock_app();
    let init_funds = [Coin::new(5_000_000_000_000u128, ORAI_DENOM)];
    let accounts = router.init_accounts(&init_funds, 1).unwrap();
    let owner = &accounts[0];
    let wasm = Wasm::new(&router);

    let cw20_ics20_id = wasm
        .store_code(WASM_BYTES, None, owner)
        .unwrap()
        .data
        .code_id;

    let allowlist: Vec<AllowMsg> = vec![];

    // arrange
    let addr1 = Addr::unchecked("addr1");
    let gov_cw20_ics20 = owner.address();

    // ibc stuff
    let src_ibc_endpoint = IbcEndpoint {
        port_id: REMOTE_PORT.to_string(),
        channel_id: "channel-0".to_string(),
    };

    let local_channel_id = "channel-0".to_string();

    let native_denom = "orai";
    let asset_info = AssetInfo::Token {
        contract_addr: Addr::unchecked("cw20:oraifoobarhelloworld".to_string()),
    };
    let remote_decimals = 18u8;
    let asset_info_decimals = 18u8;

    let cw20_ics20_init_msg = InitMsg {
        default_gas_limit: Some(20000000u64),
        default_timeout: DEFAULT_TIMEOUT,
        gov_contract: gov_cw20_ics20.clone(),
        allowlist,
        swap_router_contract: "router".to_string(),
        converter_contract: "converter".to_string(),
    };

    let cw20_ics20_contract = wasm
        .instantiate(
            cw20_ics20_id,
            &cw20_ics20_init_msg,
            Some(gov_cw20_ics20.as_str()),
            Some("cw20_ics20"),
            &[],
            owner,
        )
        .unwrap()
        .data
        .address;

    // update receiver contract

    let update_allow_msg = ExecuteMsg::UpdateMappingPair(UpdatePairMsg {
        local_channel_id: local_channel_id.clone(),
        denom: native_denom.to_string(),
        local_asset_info: asset_info.clone(),
        remote_decimals,
        local_asset_info_decimals: asset_info_decimals,
    });
    wasm.execute(&cw20_ics20_contract, &update_allow_msg, &[], owner)
        .unwrap();

    (
        router,
        addr1,
        gov_cw20_ics20,
        src_ibc_endpoint,
        local_channel_id,
        native_denom.to_string(),
        asset_info.to_string(),
        remote_decimals,
    )
}

#[test]
// cw3 multisig account can control cw20 admin actions
fn initialize_valid_successful_cw20_ics20_and_receiver_contract() {
    initialize_basic_data_for_testings();
}

// #[test]
// // cw3 multisig account can control cw20 admin actions
// fn on_ibc_receive_invalid_submsg_when_calling_allow_contract_should_undo_increase_channel_balance()
// {
//     let (
//         router,
//         addr1,
//         gov_cw20_ics20,
//         src_ibc_endpoint,
//         dest_ibc_endpoint,
//         native_denom,
//         cw20_denom,
//         remote_decimals,
//         receiver_contract,
//     ) = initialize_basic_data_for_testings();

//     let amount = 1u128;
//     let remote_sender = Addr::unchecked("remote_sender");
//     let local_receiver = Addr::unchecked("local_receiver");

//     let recv_packet = mock_receive_packet(
//         &dest_ibc_endpoint.channel_id,
//         src_ibc_endpoint.channel_id.as_str(),
//         amount,
//         &native_denom,
//         remote_sender.as_str(),
//         local_receiver.as_str(),
//     );
//     let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
// }
