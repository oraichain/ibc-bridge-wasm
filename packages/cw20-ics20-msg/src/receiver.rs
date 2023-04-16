use cosmwasm_schema::cw_serde;

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
        let mut new_destination: DestinationInfo = DestinationInfo {
            receiver: self.receiver.clone(),
            destination_channel: self.destination_channel.clone(),
            destination_denom: self.destination_denom.clone(),
        };
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
}
