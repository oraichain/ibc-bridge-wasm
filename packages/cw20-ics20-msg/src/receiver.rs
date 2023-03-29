use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct DestinationInfo {
    pub receiver: String,
    pub destination_channel: String,
    pub destination_denom: String,
}

impl DestinationInfo {
    // string format: <destination-channel>/<receiver>:<denom>
    pub fn from_str(value: &str) -> Self {
        let (destination, denom) = match value.split_once(':') {
            Some((destination, denom)) => (destination, denom),
            None => (value, ""),
        };

        let (channel, receiver) = match destination.split_once('/') {
            Some((channel, receiver)) => (channel, receiver),
            None => match destination.find("0x") {
                Some(ind) => (
                    destination.get(0..ind).unwrap_or_default(),
                    destination.get(ind..).unwrap_or_default(),
                ),
                None => ("", destination),
            },
        };

        Self {
            receiver: receiver.to_string(),
            destination_channel: channel.to_string(),
            destination_denom: denom.to_string(),
        }
    }
}

#[test]
fn parse_destination_info() {
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
    let d2 =
        DestinationInfo::from_str("trx-mainnet0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64:usdt");
    assert_eq!(
        d2,
        DestinationInfo {
            receiver: "0x73Ddc880916021EFC4754Cb42B53db6EAB1f9D64".to_string(),
            destination_channel: "trx-mainnet".to_string(),
            destination_denom: "usdt".to_string()
        }
    );

    let d3 = DestinationInfo::from_str("orai14n3tx8s5ftzhlxvq0w5962v60vd82h30rha573");
    assert_eq!(
        d3,
        DestinationInfo {
            receiver: "orai14n3tx8s5ftzhlxvq0w5962v60vd82h30rha573".to_string(),
            destination_channel: "".to_string(),
            destination_denom: "".to_string()
        }
    );
}
