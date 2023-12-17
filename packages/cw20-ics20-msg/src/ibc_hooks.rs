use crate::helper::to_orai_bridge_address;
use anybuf::Bufany;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Api, Binary, CanonicalAddr, StdError, StdResult};

#[cw_serde]
pub enum HookMethods {
    UniversalSwap,
}

#[cw_serde]
pub struct IbcHooksUniversalSwap {
    pub receiver: String,             // receiver on Oraichain
    pub destination_receiver: String, // destination  receiver
    pub destination_channel: String,
    pub destination_denom: String,
    pub bridge_receiver: String, // used for case where destination is evm, this address will be the orai bridge address
}

impl IbcHooksUniversalSwap {
    pub fn from_binary(api: &dyn Api, value: &Binary) -> StdResult<Self> {
        let deserialized = Bufany::deserialize(&value).unwrap();
        let receiver = api
            .addr_humanize(&CanonicalAddr::from(deserialized.bytes(1).unwrap()))
            .unwrap()
            .to_string();
        let destination_receiver = deserialized.string(2).unwrap();
        let destination_channel = deserialized.string(3).unwrap();
        let destination_denom = deserialized.string(4).unwrap();
        let bridge_receiver = to_orai_bridge_address(&receiver).unwrap();

        // Always require destination.receiver
        if destination_receiver.is_empty() {
            return Err(StdError::generic_err(
                "Require destination receiver in memo",
            ));
        }

        Ok(Self {
            receiver: receiver.clone(),
            destination_receiver,
            destination_channel,
            destination_denom,
            bridge_receiver,
        })
    }
}

#[cfg(test)]
mod test {

    use cosmwasm_std::Binary;
    use cosmwasm_testing_util::mock::MockApi;

    use crate::ibc_hooks::IbcHooksUniversalSwap;

    #[test]
    fn test_parse_ibc_hools_universal_swap() {
        let memo = String::from("ChTsBUd+mFGWclLECUb6spUV/eCArxI5dHJvbnRyeC1tYWlubmV0MHhiMmM1MWViZDk4NTc2YmYxMmJlZWNlMDZlMzhlNGQ0ODYxNDEwODYxGgpjaGFubmVsLTI5IitvcmFpMTJoemp4Zmg3N3dsNTcyZ2R6Y3QyZnh2MmFyeGN3aDZneWtjN3Fo");

        let res = IbcHooksUniversalSwap::from_binary(
            &MockApi::default(),
            &Binary::from_base64(&memo).unwrap(),
        )
        .unwrap();

        assert_eq!(
            res,
            IbcHooksUniversalSwap {
                receiver: "orai1asz5wl5c2xt8y5kyp9r04v54zh77pq90fhchjq".to_string(),
                destination_receiver: "trontrx-mainnet0xb2c51ebd98576bf12beece06e38e4d4861410861"
                    .to_string(),
                destination_channel: "channel-29".to_string(),
                destination_denom: "orai12hzjxfh77wl572gdzct2fxv2arxcwh6gykc7qh".to_string(),
                bridge_receiver: "oraib1asz5wl5c2xt8y5kyp9r04v54zh77pq907kumrr".to_string()
            }
        )
    }
}
