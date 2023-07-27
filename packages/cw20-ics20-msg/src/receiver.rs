use cosmwasm_schema::cw_serde;

use crate::helper::get_prefix_decode_bech32;

#[cw_serde]
pub struct DestinationInfo {
    pub receiver: String,
    pub destination_channel: String,
    /// destination denom can be in cw20 form or ibc/<hash>
    pub destination_denom: String,
}

impl DestinationInfo {
    // destination string format: <destination-channel>/<receiver>:<denom>
    pub fn from_str(value: &str) -> Self {
        let (destination, denom) = match value.split_once(':') {
            Some((destination, denom)) => (destination, denom),
            None => (value, ""),
        };

        let (channel, receiver) = match destination.split_once('/') {
            Some((channel, receiver)) => (channel, receiver),
            None => ("", destination),
        };

        Self {
            receiver: receiver.to_string(),
            destination_channel: channel.to_string(),
            destination_denom: denom.to_string(),
        }
    }

    pub fn is_receiver_evm_based(&self) -> (bool, Self) {
        let mut new_destination: DestinationInfo = DestinationInfo { ..self.clone() };
        match self.receiver.split_once("0x") {
            Some((evm_prefix, address)) => {
                // has to have evm_prefix, otherwise we would not be able to know the real denom
                if evm_prefix.is_empty() {
                    return (false, new_destination);
                }
                // after spliting (removing 0x) => size 40 for eth address
                if address.len() != 40usize {
                    return (false, new_destination);
                }
                // we store evm-preifx as destination channel so we can filter in the pair mapping based on asset info
                new_destination.destination_channel = evm_prefix.to_string();
                (true, new_destination)
            }
            None => (false, new_destination),
        }
    }

    pub fn is_receiver_cosmos_based(&self) -> (bool, Self) {
        let mut new_destination: DestinationInfo = DestinationInfo { ..self.clone() };
        match get_prefix_decode_bech32(&new_destination.receiver).ok() {
            None => (false, new_destination),
            Some(prefix) => {
                if prefix.is_empty() {
                    return (false, new_destination);
                }
                new_destination.destination_channel = prefix.to_string();
                (true, new_destination)
            }
        }
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
    let (is_evm_based, d1) = d1.is_receiver_evm_based();
    assert_eq!(true, is_evm_based);
    assert_eq!("foobar".to_string(), d1.destination_channel);
    assert_eq!(
        "foobar0x3C5C6b570C1DA469E8B24A2E8Ed33c278bDA3222".to_string(),
        d1.receiver
    );
}

#[test]
fn test_is_cosmos_based() {
    let d1 = DestinationInfo::from_str("foo");
    assert_eq!(false, d1.is_receiver_cosmos_based().0);

    let d1 = DestinationInfo::from_str("channel-15/foo:usdt");
    assert_eq!(false, d1.is_receiver_cosmos_based().0);

    let d1 =
        DestinationInfo::from_str("channel-15/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz:usdt");
    let result = d1.is_receiver_cosmos_based();
    assert_eq!(true, result.0);
    assert_eq!("cosmos", result.1.destination_channel);

    let d1 =
        DestinationInfo::from_str("channel-15/akash1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejjpn5xp:usdt");
    let result = d1.is_receiver_cosmos_based();
    assert_eq!(true, result.0);
    assert_eq!("akash", result.1.destination_channel);

    let d1 =
        DestinationInfo::from_str("channel-15/bostrom1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejuf2qpu:usdt");
    let result = d1.is_receiver_cosmos_based();
    assert_eq!(true, result.0);
    assert_eq!("bostrom", result.1.destination_channel);
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
}

#[test]
fn test_parse_destination_info() {
    // swap to orai then orai to atom, then use swapped amount to transfer ibc to destination
    let d1 =
        DestinationInfo::from_str("channel-15/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz:atom");
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
    let d3 = DestinationInfo::from_str("channel-15/cosmos14n3tx8s5ftzhlxvq0w5962v60vd82h30sythlz");
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
                "ibc/A2E2EEC9057A4A1C2C0A6A4C78B0239118DF5F278830F50B4A6BDD7A66506B78".to_string()
        }
    );
}
