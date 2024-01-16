use cosmwasm_std::{Coin, Uint128};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug, Default)]
pub struct Ics20Packet {
    /// amount of tokens to transfer is encoded as a string, but limited to u64 max
    pub amount: Uint128,
    /// the token denomination to be transferred
    pub denom: String,
    /// the recipient address on the destination chain
    pub receiver: String,
    /// the sender address
    pub sender: String,
    /// optional memo for the IBC transfer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Deal {
    pub seller: String,
    pub buyer: String,
    pub coin_a: Coin,
    pub coin_b: Coin,
    pub expiry: u64,
    pub finished: bool,
    pub seller_deposited: bool,
    pub buyer_deposited: bool,
    pub seller_withdrew: bool,
    pub buyer_withdrew: bool,
    pub channel_id_recieved_a: String,
    pub channel_id_recieved_b: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub deals: Vec<Deal>,
}

pub const STATE: Item<State> = Item::new("AWESOME");
