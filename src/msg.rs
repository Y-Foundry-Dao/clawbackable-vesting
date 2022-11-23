use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Order, Uint128};
use cw20::Cw20ReceiveMsg;

#[cw_serde]
pub enum OrderBy {
    Asc,
    Desc,
}

#[allow(clippy::from_over_into)]
impl Into<Order> for OrderBy {
    fn into(self) -> Order {
        if self == OrderBy::Asc {
            Order::Ascending
        } else {
            Order::Descending
        }
    }
}

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub token_addr: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Claim {
        recipient: Option<String>,
        amount: Option<Uint128>,
    },
    Clawback {
        recipient: Addr,
    },
    Receive(Cw20ReceiveMsg),
    ProposeNewOwner {
        owner: String,
        expires_in: u64,
    },
    DropOwnershipProposal {},
    ClaimOwnership {},
}

#[cw_serde]
pub enum Cw20HookMsg {
    RegisterVestingAccounts {
        vesting_accounts: Vec<VestingAccount>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(VestingAccountResponse)]
    VestingAccount { address: String },
    #[returns(VestingAccountsResponse)]
    VestingAccounts {
        start_after: Option<String>,
        limit: Option<u32>,
        order_by: Option<OrderBy>,
    },
    #[returns(Uint128)]
    AvailableAmount { address: String },
    #[returns(u64)]
    Timestamp {},
}

#[cw_serde]
pub struct VestingAccount {
    pub address: String,
    pub schedules: Vec<VestingSchedule>,
    pub clawbackable: Option<bool>,
}

#[cw_serde]
pub struct VestingSchedule {
    pub start_point: VestingSchedulePoint,
    pub end_point: Option<VestingSchedulePoint>,
}

#[cw_serde]
pub struct VestingSchedulePoint {
    pub time: u64,
    pub amount: Uint128,
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: Addr,
    pub token_addr: Addr,
}

#[cw_serde]
pub struct VestingInfo {
    pub schedules: Vec<VestingSchedule>,
    pub released_amount: Uint128,
    pub clawbackable: Option<bool>,
}

#[cw_serde]
pub struct VestingAccountResponse {
    pub address: Addr,
    pub info: VestingInfo,
}

#[cw_serde]
pub struct VestingAccountsResponse {
    pub vesting_accounts: Vec<VestingAccountResponse>,
}

#[cw_serde]
pub struct MigrateMsg {}
