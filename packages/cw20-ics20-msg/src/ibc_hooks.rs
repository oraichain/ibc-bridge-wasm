use crate::helper::to_orai_bridge_address;
use anybuf::Bufany;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Api, Binary, StdError, StdResult};

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
        let deserialized = match Bufany::deserialize(&value) {
            Ok(val) => val,
            Err(err) => {
                return Err(StdError::generic_err(format!(
                    "Error on deserialize: {:?}",
                    err
                )))
            }
        };

        let receiver = api
            .addr_humanize(
                &deserialized
                    .bytes(1)
                    .ok_or_else(|| StdError::generic_err("Error on deserialize receiver"))?
                    .into(),
            )?
            .to_string();
        let destination_receiver = deserialized
            .string(2)
            .ok_or_else(|| StdError::generic_err("Error on deserialize destination_receiver"))?;

        let destination_channel = deserialized
            .string(3)
            .ok_or_else(|| StdError::generic_err("Error on deserialize destination_channel"))?;

        let destination_denom = deserialized
            .string(4)
            .ok_or_else(|| StdError::generic_err("Error on deserialize destination_denom"))?;

        let bridge_receiver = match to_orai_bridge_address(&receiver) {
            Ok(val) => val,
            Err(err) => {
                return Err(StdError::generic_err(format!(
                    "Error on convert to orai bridge address: {:?}",
                    err
                )))
            }
        };

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

    use anybuf::Anybuf;
    use cosmwasm_std::{Api, Binary};
    use cosmwasm_testing_util::mock::MockApi;

    use crate::ibc_hooks::IbcHooksUniversalSwap;

    #[test]
    fn test_parse_ibc_hools_universal_swap() {
        let mock_api = MockApi::default();

        let memo = Binary::from(
            Anybuf::new()
                .append_bytes(
                    1,
                    mock_api
                        .addr_canonicalize("orai1asz5wl5c2xt8y5kyp9r04v54zh77pq90fhchjq")
                        .unwrap()
                        .as_slice(),
                ) // receiver on Oraichain
                .append_string(
                    2,
                    "trontrx-mainnet0xb2c51ebd98576bf12beece06e38e4d4861410861",
                ) // destination receiver
                .append_string(3, "channel-29") // destination channel
                .append_string(
                    4,
                    "orai12hzjxfh77wl572gdzct2fxv2arxcwh6gykc7qh", //destination denom
                )
                .as_bytes(),
        )
        .to_base64();

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
