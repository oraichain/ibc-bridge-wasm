use anybuf::Bufany;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, StdError, StdResult};

use crate::helper::get_prefix_decode_bech32;

#[cw_serde]
pub struct DestinationInfo {
    pub receiver: String,
    pub destination_channel: String,
    /// destination denom can be in cw20 form or ibc/<hash>
    pub destination_denom: String,
}

impl Default for DestinationInfo {
    fn default() -> Self {
        DestinationInfo {
            receiver: "".to_string(),
            destination_channel: "".to_string(),
            destination_denom: "".to_string(),
        }
    }
}

impl DestinationInfo {
    // destination string format: <destination-channel>/<receiver>:<denom>
    pub fn from_str(value: &str) -> Self {
        let (destination, denom) = value.split_once(':').unwrap_or((value, ""));
        let (channel, receiver) = destination.split_once('/').unwrap_or(("", destination));

        Self {
            receiver: receiver.to_string(),
            destination_channel: channel.to_string(),
            destination_denom: denom.to_string(),
        }
    }

    pub fn from_base64(encoded: &str) -> StdResult<Self> {
        DestinationInfo::from_binary(&Binary::from_base64(encoded)?)
    }

    fn from_binary(value: &Binary) -> StdResult<Self> {
        let deserialized = Bufany::deserialize(&value)
            .map_err(|err| StdError::generic_err(format!("Error on deserialize: {:?}", err)))?;

        let destination_receiver = deserialized
            .string(1)
            .ok_or_else(|| StdError::generic_err("Error on deserialize destination_receiver"))?;

        let destination_channel = deserialized
            .string(2)
            .ok_or_else(|| StdError::generic_err("Error on deserialize destination_channel"))?;

        let destination_denom = deserialized
            .string(3)
            .ok_or_else(|| StdError::generic_err("Error on deserialize destination_denom"))?;

        Ok(Self {
            receiver: destination_receiver,
            destination_channel,
            destination_denom,
        })
    }

    pub fn is_receiver_evm_based(&self) -> (bool, String) {
        match self.receiver.split_once("0x") {
            Some((evm_prefix, address)) => {
                // has to have evm_prefix, otherwise we would not be able to know the real denom
                if evm_prefix.is_empty() {
                    return (false, "".to_string());
                }
                // after spliting (removing 0x) => size 40 for eth address
                if address.len() != 40usize {
                    return (false, "".to_string());
                }
                // we store evm-preifx as destination channel so we can filter in the pair mapping based on asset info
                (true, evm_prefix.to_string())
            }
            None => (false, "".to_string()),
        }
    }

    pub fn is_receiver_cosmos_based(&self) -> bool {
        get_prefix_decode_bech32(&self.receiver)
            .unwrap_or_default() // empty string if error
            .len()
            > 0
    }
}

#[test]
fn test_is_evm_based() {
    let d1 = DestinationInfo::from_str("cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
    assert_eq!(false, d1.is_receiver_evm_based().0);
    let d1 = DestinationInfo::from_str("0x3C5C6b570C1DA469E8B24A2E8Ed33c278bDA3222");
    // false here because we need the evm-prefix as well!
    assert_eq!(false, d1.is_receiver_evm_based().0);
    let d1 = DestinationInfo::from_str("foobar0x3C5C6b570C1DA469E8B24A2E8Ed33c278b");
    // false here because of the wrong eth address, not enough in length
    assert_eq!(false, d1.is_receiver_evm_based().0);
    let d1 = DestinationInfo::from_str(
        "channel-15/foobar0x3C5C6b570C1DA469E8B24A2E8Ed33c278bDA3222:usdt",
    );
    let (is_evm_based, prefix) = d1.is_receiver_evm_based();
    assert_eq!(true, is_evm_based);
    assert_eq!("foobar".to_string(), prefix);
    assert_eq!(
        "foobar0x3C5C6b570C1DA469E8B24A2E8Ed33c278bDA3222".to_string(),
        d1.receiver
    );
}

#[test]
fn test_is_cosmos_based() {
    let d1 = DestinationInfo::from_str("foo");
    assert_eq!(false, d1.is_receiver_cosmos_based());

    let d1 = DestinationInfo::from_str("channel-15/foo:usdt");
    assert_eq!(false, d1.is_receiver_cosmos_based());

    let d1 =
        DestinationInfo::from_str("channel-15/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz:usdt");
    let result = d1.is_receiver_cosmos_based();
    assert_eq!(true, result);

    let d1 =
        DestinationInfo::from_str("channel-15/akash1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejjpn5xp:usdt");
    let result = d1.is_receiver_cosmos_based();
    assert_eq!(true, result);

    let d1 =
        DestinationInfo::from_str("channel-15/bostrom1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejuf2qpu:usdt");
    let result = d1.is_receiver_cosmos_based();
    assert_eq!(true, result);

    let d1 = DestinationInfo::from_str("channel-124/cosmos1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejl67nlm:orai17l2zk3arrx0a0fyuneyx8raln68622a2lrsz8ph75u7gw9tgz3esayqryf");
    let result = d1.is_receiver_cosmos_based();
    assert_eq!(true, result);
}

#[test]
fn test_destination_info_from_str() {
    let d1 = DestinationInfo::from_str("cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
    assert_eq!(d1.destination_channel, "");
    assert_eq!(d1.receiver, "cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
    assert_eq!(d1.destination_denom, "");

    let d1 = DestinationInfo::from_str("cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz:foo");
    assert_eq!(d1.destination_channel, "");
    assert_eq!(d1.receiver, "cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
    assert_eq!(d1.destination_denom, "foo");

    let d1 = DestinationInfo::from_str("foo/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
    assert_eq!(d1.destination_channel, "foo");
    assert_eq!(d1.receiver, "cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
    assert_eq!(d1.destination_denom, "");

    let d1 = DestinationInfo::from_str("foo/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz:bar");
    assert_eq!(d1.destination_channel, "foo");
    assert_eq!(d1.receiver, "cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
    assert_eq!(d1.destination_denom, "bar");

    let d1 = DestinationInfo::from_str("");
    assert_eq!(d1.destination_channel, "");
    assert_eq!(d1.receiver, "");
    assert_eq!(d1.destination_denom, "");
}

#[cfg(test)]
mod tests {
    use anybuf::Anybuf;
    use cosmwasm_std::{Binary, StdError};

    use crate::receiver::DestinationInfo;

    #[test]
    fn test_parse_destination_info() {
        // swap to orai then orai to atom, then use swapped amount to transfer ibc to destination
        let d1 = DestinationInfo::from_str(
            "channel-15/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz:atom",
        );
        assert_eq!(
            d1,
            DestinationInfo {
                receiver: "cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz".to_string(),
                destination_channel: "channel-15".to_string(),
                destination_denom: "atom".to_string()
            }
        );
        // swap to orai then orai to usdt with 'to' as the receiver when swapping, then we're done
        let d2 = DestinationInfo::from_str("orai14n3tx8s5ftzhlxvq0w5962v60vd82h30rha573:usdt");
        assert_eq!(
            d2,
            DestinationInfo {
                receiver: "orai14n3tx8s5ftzhlxvq0w5962v60vd82h30rha573".to_string(),
                destination_channel: "".to_string(),
                destination_denom: "usdt".to_string()
            }
        );
        // this case returns an error (because it has channel but no destination denom)
        let d3 =
            DestinationInfo::from_str("channel-15/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
        assert_eq!(
            d3,
            DestinationInfo {
                receiver: "cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz".to_string(),
                destination_channel: "channel-15".to_string(),
                destination_denom: "".to_string()
            }
        );
        let d4 =
            DestinationInfo::from_str("trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64:usdt");
        assert_eq!(
            d4,
            DestinationInfo {
                receiver: "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string(),
                destination_channel: "".to_string(),
                destination_denom: "usdt".to_string()
            }
        );

        let d5 = DestinationInfo::from_str("orai14n3tx8s5ftzhlxvq0w5962v60vd82h30rha573");
        assert_eq!(
            d5,
            DestinationInfo {
                receiver: "orai14n3tx8s5ftzhlxvq0w5962v60vd82h30rha573".to_string(),
                destination_channel: "".to_string(),
                destination_denom: "".to_string()
            }
        );

        let d6 = DestinationInfo::from_str(
            "channel-5/trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64:usdt",
        );
        assert_eq!(
            d6,
            DestinationInfo {
                receiver: "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string(),
                destination_channel: "channel-5".to_string(),
                destination_denom: "usdt".to_string()
            }
        );
        // ibc hash case
        let d7 = DestinationInfo::from_str("channel-5/trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64:ibc/A2E2EEC9057A4A1C2C0A6A4C78B0239118DF5F278830F50B4A6BDD7A66506B78");
        assert_eq!(
            d7,
            DestinationInfo {
                receiver: "trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string(),
                destination_channel: "channel-5".to_string(),
                destination_denom:
                    "ibc/A2E2EEC9057A4A1C2C0A6A4C78B0239118DF5F278830F50B4A6BDD7A66506B78"
                        .to_string()
            }
        );
        let d8 = DestinationInfo::from_str("channel-124/cosmos1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejl67nlm:orai17l2zk3arrx0a0fyuneyx8raln68622a2lrsz8ph75u7gw9tgz3esayqryf");
        assert_eq!(
            d8,
            DestinationInfo {
                receiver: "cosmos1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejl67nlm".to_string(),
                destination_channel: "channel-124".to_string(),
                destination_denom:
                    "orai17l2zk3arrx0a0fyuneyx8raln68622a2lrsz8ph75u7gw9tgz3esayqryf".to_string(),
            }
        )
    }

    #[test]
    fn test_parse_destination_from_binary_invalid_type() {
        let memo = Binary::from(
            Anybuf::new()
                .append_int32(1, 100) // destination receiver
                .append_string(2, "channel-170") // destination channel
                .append_string(
                    3, "orai", //destination denom
                )
                .as_bytes(),
        )
        .to_base64();

        let res = DestinationInfo::from_base64(&memo);

        assert_eq!(
            res.unwrap_err(),
            StdError::generic_err("Error on deserialize destination_receiver")
        );

        let memo = Binary::from(
            Anybuf::new()
                .append_string(1, "orai1asz5wl5c2xt8y5kyp9r04v54zh77pq90fhchjq") // destination receiver
                .append_int32(2, 100) // destination channel
                .append_string(
                    3, "orai", //destination denom
                )
                .as_bytes(),
        )
        .to_base64();

        let res = DestinationInfo::from_base64(&memo);
        assert_eq!(
            res.unwrap_err(),
            StdError::generic_err("Error on deserialize destination_channel")
        );

        let memo = Binary::from(
            Anybuf::new()
                .append_string(1, "orai1asz5wl5c2xt8y5kyp9r04v54zh77pq90fhchjq") // destination receiver
                .append_string(2, "channel-170") // destination channel
                .append_int32(
                    3, 2, //destination denom
                )
                .as_bytes(),
        )
        .to_base64();

        let res = DestinationInfo::from_base64(&memo);
        assert_eq!(
            res.unwrap_err(),
            StdError::generic_err("Error on deserialize destination_denom")
        );
    }

    #[test]
    fn test_parse_destination_from_binary_valid() {
        let memo = Binary::from(
            Anybuf::new()
                .append_string(1, "orai1asz5wl5c2xt8y5kyp9r04v54zh77pq90fhchjq") // destination receiver
                .append_string(2, "channel-170") // destination channel
                .append_string(
                    3, "orai", //destination denom
                )
                .as_bytes(),
        )
        .to_base64();
        println!("{:?}", memo);

        let res = DestinationInfo::from_base64(&memo).unwrap();
        assert_eq!(
            res,
            DestinationInfo {
                receiver: "orai1asz5wl5c2xt8y5kyp9r04v54zh77pq90fhchjq".to_string(),
                destination_channel: "channel-170".to_string(),
                destination_denom: "orai".to_string()
            }
        );
    }
}
