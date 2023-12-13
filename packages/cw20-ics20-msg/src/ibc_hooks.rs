use cosmwasm_std::{Binary, StdError, StdResult};

use crate::receiver::BridgeInfo;

pub enum HookTypes {
    ConvertToken,
    BridgeInfo(BridgeInfo),
    DestinationMemo(String),
}

pub fn parse_hooks_msg(args: &Binary) -> StdResult<Vec<HookTypes>> {
    let mut hooks = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index] {
            // 0 : ConvertToken
            0 => {
                index += 1;
                hooks.push(HookTypes::ConvertToken);
                index += 1;
            }
            // 1: Bridge Info
            1 => {
                index += 1;

                // get channel
                let channel_len = args[index] as usize;
                let channel =
                    String::from_utf8((&args[index + 1..index + channel_len + 1]).to_vec())?;
                index += channel_len + 1;

                // get sender
                let sender_len = args[index] as usize;
                let sender =
                    String::from_utf8((&args[index + 1..index + sender_len + 1]).to_vec())?;
                index += sender_len + 1;

                // get receiver
                let receiver_len = args[index] as usize;
                let receiver =
                    String::from_utf8((&args[index + 1..index + receiver_len + 1]).to_vec())?;
                index += receiver_len + 1;

                hooks.push(HookTypes::BridgeInfo(BridgeInfo {
                    channel,
                    sender,
                    receiver,
                }));
            }
            // 2: Destination  Gravity Bridge Info
            2 => {
                index += 1;
                let destination_length = args[index] as usize;
                let destination_memo =
                    String::from_utf8((&args[index + 1..index + destination_length + 1]).to_vec())?;
                index += destination_length;

                hooks.push(HookTypes::DestinationMemo(destination_memo))
            }
            _ => return Err(StdError::generic_err("Invalid hooks methods")),
        }
    }

    Ok(hooks)
}
