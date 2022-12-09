use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Coin};

use crate::state::State;

#[cw_serde]
pub struct InstantiateMsg {
    // owner and creator come from env
    // collateral comes form env
    pub counter_offer: Vec<Coin>,
    pub expires: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Owner can transfer to a new owner
    Transfer { recipient: Addr },
    /// Owner can post counter_offer on unexpired option to execute and get the collateral
    Execute {},
    /// Burn will release collateral if expired
    Burn {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(ConfigResponse)]
    Config {},
}

// We define a custom struct for each query response
// #[cw_serde]
pub type ConfigResponse = State;
