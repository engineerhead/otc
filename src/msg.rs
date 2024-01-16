use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Coin;

use crate::state::{Deal, Ics20Packet};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    CreateDeal {
        seller: String,
        buyer: String,
        coin_a: Coin,
        coin_b: Coin,
        expiry: u64,
    },
    Deposit {},
    CompleteDeal {},
    Withdraw {},
    Reset {},
}

#[cw_serde]
pub enum IbcExecuteMsg {
    Deposit { packet20: Ics20Packet },
    Withdraw { packet20: Ics20Packet },
    CompleteDeal { packet20: Ics20Packet },
}

#[cw_serde]
pub struct BalancesResponse {
    pub balances: Vec<Coin>,
}

#[cw_serde]
pub struct DealResponse {
    deal: Deal,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(BalancesResponse)]
    GetBalances { address: String },
    #[returns(DealResponse)]
    GetDeal { id: u64 },
}
