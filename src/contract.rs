use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};

use crate::state::{
    read_vesting_infos, Config, OwnershipProposal, CONFIG, OWNERSHIP_PROPOSAL, VESTING_INFO,
};

use crate::error::ContractError;

use crate::msg::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, OrderBy, QueryMsg,
    VestingAccount, VestingAccountResponse, VestingAccountsResponse, VestingInfo, VestingSchedule,
};
use crate::util::{addr_opt_validate, addr_validate_to_lower};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_storage_plus::Item;

const CONTRACT_NAME: &str = "clawbackable-vesting";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_PROPOSAL_TTL: u64 = 1209600;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CONFIG.save(
        deps.storage,
        &Config {
            owner: addr_validate_to_lower(deps.api, &msg.owner)?,
            token_addr: addr_validate_to_lower(deps.api, &msg.token_addr)?,
        },
    )?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Claim { recipient, amount } => claim(deps, env, info, recipient, amount),
        ExecuteMsg::Clawback { recipient } => clawback(deps, env, info, recipient),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            propose_new_owner(deps, info, env, owner, expires_in, OWNERSHIP_PROPOSAL)
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            drop_ownership_proposal(deps, info, OWNERSHIP_PROPOSAL)
        }
        ExecuteMsg::ClaimOwnership {} => claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL),
    }
}

fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if cw20_msg.sender != config.owner || info.sender != config.token_addr {
        return Err(ContractError::Unauthorized {});
    }

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::RegisterVestingAccounts { vesting_accounts } => {
            register_vesting_accounts(deps, vesting_accounts, cw20_msg.amount)
        }
    }
}

pub fn propose_new_owner(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    new_owner: String,
    expires_in: u64,
    proposal: Item<OwnershipProposal>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let new_owner = addr_validate_to_lower(deps.api, new_owner.as_str())?;

    // Check that the new owner is not the same as the current one
    if new_owner == config.owner {
        return Err(ContractError::Std(StdError::generic_err(
            "New owner cannot be same",
        )));
    }

    if MAX_PROPOSAL_TTL < expires_in {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Parameter expires_in cannot be higher than {}",
            MAX_PROPOSAL_TTL
        ))));
    }

    proposal.save(
        deps.storage,
        &OwnershipProposal {
            owner: new_owner.clone(),
            ttl: env.block.time.seconds() + expires_in,
        },
    )?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "propose_new_owner"),
        attr("new_owner", new_owner),
    ]))
}

pub fn drop_ownership_proposal(
    deps: DepsMut,
    info: MessageInfo,
    proposal: Item<OwnershipProposal>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    proposal.remove(deps.storage);

    Ok(Response::new().add_attributes(vec![attr("action", "drop_ownership_proposal")]))
}

pub fn claim_ownership(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    proposal: Item<OwnershipProposal>,
) -> Result<Response, ContractError> {
    let p = proposal
        .load(deps.storage)
        .map_err(|_| ContractError::Std(StdError::generic_err("Ownership proposal not found")))?;

    // Check the sender
    if info.sender != p.owner {
        return Err(ContractError::Unauthorized {});
    }

    if env.block.time.seconds() > p.ttl {
        return Err(ContractError::Std(StdError::generic_err(
            "Ownership proposal expired",
        )));
    }

    proposal.remove(deps.storage);

    CONFIG.update::<_, StdError>(deps.storage, |mut v| {
        v.owner = p.owner.clone();
        Ok(v)
    })?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "claim_ownership"),
        attr("new_owner", p.owner),
    ]))
}

pub fn register_vesting_accounts(
    deps: DepsMut,
    vesting_accounts: Vec<VestingAccount>,
    cw20_amount: Uint128,
) -> Result<Response, ContractError> {
    let response = Response::new();

    let mut to_deposit = Uint128::zero();

    for mut vesting_account in vesting_accounts {
        let mut released_amount = Uint128::zero();
        let account_address = addr_validate_to_lower(deps.api, &vesting_account.address)?;

        assert_vesting_schedules(&account_address, &vesting_account.schedules)?;

        for sch in &vesting_account.schedules {
            let amount = if let Some(end_point) = &sch.end_point {
                end_point.amount
            } else {
                sch.start_point.amount
            };
            to_deposit = to_deposit.checked_add(amount)?;
        }

        if let Some(mut old_info) = VESTING_INFO.may_load(deps.storage, &account_address)? {
            released_amount = old_info.released_amount;
            vesting_account.schedules.append(&mut old_info.schedules);
        }

        VESTING_INFO.save(
            deps.storage,
            &account_address,
            &VestingInfo {
                schedules: vesting_account.schedules,
                released_amount,
                clawbackable: vesting_account.clawbackable,
            },
        )?;
    }

    if to_deposit != cw20_amount {
        return Err(ContractError::VestingScheduleAmountError {});
    }

    Ok(response.add_attributes({
        vec![
            attr("action", "register_vesting_accounts"),
            attr("deposited", to_deposit),
        ]
    }))
}

fn assert_vesting_schedules(
    addr: &Addr,
    vesting_schedules: &[VestingSchedule],
) -> Result<(), ContractError> {
    for sch in vesting_schedules {
        if let Some(end_point) = &sch.end_point {
            if !(sch.start_point.time < end_point.time && sch.start_point.amount < end_point.amount)
            {
                return Err(ContractError::VestingScheduleError(addr.to_string()));
            }
        }
    }

    Ok(())
}

pub fn claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Option<String>,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut vesting_info = VESTING_INFO.load(deps.storage, &info.sender)?;

    let available_amount = compute_available_amount(env.block.time.seconds(), &vesting_info)?;

    let claim_amount = if let Some(a) = amount {
        if a > available_amount {
            return Err(ContractError::AmountIsNotAvailable {});
        };
        a
    } else {
        available_amount
    };

    let mut response = Response::new();

    if !claim_amount.is_zero() {
        response = response.add_submessage(SubMsg::new(WasmMsg::Execute {
            contract_addr: config.token_addr.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient.unwrap_or_else(|| info.sender.to_string()),
                amount: claim_amount,
            })?,
        }));

        vesting_info.released_amount = vesting_info.released_amount.checked_add(claim_amount)?;
        VESTING_INFO.save(deps.storage, &info.sender, &vesting_info)?;
    };

    Ok(response.add_attributes(vec![
        attr("action", "claim"),
        attr("address", &info.sender),
        attr("available_amount", available_amount),
        attr("claimed_amount", claim_amount),
    ]))
}

fn compute_available_amount(current_time: u64, vesting_info: &VestingInfo) -> StdResult<Uint128> {
    let mut available_amount: Uint128 = Uint128::zero();
    for sch in &vesting_info.schedules {
        if sch.start_point.time > current_time {
            continue;
        }

        available_amount = available_amount.checked_add(sch.start_point.amount)?;

        if let Some(end_point) = &sch.end_point {
            let passed_time = current_time.min(end_point.time) - sch.start_point.time;
            let time_period = end_point.time - sch.start_point.time;
            if passed_time != 0 && time_period != 0 {
                let release_amount = Uint128::from(passed_time).multiply_ratio(
                    end_point.amount.checked_sub(sch.start_point.amount)?,
                    time_period,
                );
                available_amount = available_amount.checked_add(release_amount)?;
            }
        }
    }

    available_amount
        .checked_sub(vesting_info.released_amount)
        .map_err(StdError::from)
}

pub fn clawback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Addr,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut vesting_info = VESTING_INFO.load(deps.storage, &recipient)?;

    if let Some(clawbackable) = vesting_info.clawbackable {
        if !clawbackable {
            return Err(ContractError::Unauthorized {});
        }
    };

    let claim_amount = compute_available_clawback_amount(env.block.time.seconds(), &vesting_info)?;

    let mut response = Response::new();

    if !claim_amount.is_zero() {
        response = response.add_submessage(SubMsg::new(WasmMsg::Execute {
            contract_addr: config.token_addr.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: claim_amount,
            })?,
        }));

        vesting_info.released_amount = vesting_info.released_amount.checked_add(claim_amount)?;
        VESTING_INFO.save(deps.storage, &info.sender, &vesting_info)?;
    };

    Ok(response.add_attributes(vec![
        attr("action", "claim"),
        attr("address", &info.sender),
        attr("claimed_amount", claim_amount),
    ]))
}

fn compute_available_clawback_amount(
    current_time: u64,
    vesting_info: &VestingInfo,
) -> StdResult<Uint128> {
    let mut available_amount: Uint128 = Uint128::zero();
    for sch in &vesting_info.schedules {
        if sch.start_point.time > current_time {
            continue;
        }

        if let Some(end_point) = &sch.end_point {
            available_amount = available_amount.checked_add(end_point.amount)?;
        } else {
            available_amount = available_amount.checked_add(sch.start_point.amount)?;
        }
    }

    available_amount
        .checked_sub(vesting_info.released_amount)
        .map_err(StdError::from)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::VestingAccount { address } => {
            Ok(to_binary(&query_vesting_account(deps, address)?)?)
        }
        QueryMsg::VestingAccounts {
            start_after,
            limit,
            order_by,
        } => Ok(to_binary(&query_vesting_accounts(
            deps,
            start_after,
            limit,
            order_by,
        )?)?),
        QueryMsg::AvailableAmount { address } => Ok(to_binary(&query_vesting_available_amount(
            deps, env, address,
        )?)?),
        QueryMsg::Timestamp {} => Ok(to_binary(&query_timestamp(env)?)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner,
        token_addr: config.token_addr,
    })
}

pub fn query_timestamp(env: Env) -> StdResult<u64> {
    Ok(env.block.time.seconds())
}

pub fn query_vesting_account(deps: Deps, address: String) -> StdResult<VestingAccountResponse> {
    let address = addr_validate_to_lower(deps.api, &address)?;
    let info = VESTING_INFO.load(deps.storage, &address)?;

    Ok(VestingAccountResponse { address, info })
}

pub fn query_vesting_accounts(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<VestingAccountsResponse> {
    let start_after = addr_opt_validate(deps.api, &start_after)?;

    let vesting_infos = read_vesting_infos(deps, start_after, limit, order_by)?;

    let vesting_accounts: Vec<_> = vesting_infos
        .into_iter()
        .map(|(address, info)| VestingAccountResponse { address, info })
        .collect();

    Ok(VestingAccountsResponse { vesting_accounts })
}

pub fn query_vesting_available_amount(deps: Deps, env: Env, address: String) -> StdResult<Uint128> {
    let address = addr_validate_to_lower(deps.api, &address)?;

    let info = VESTING_INFO.load(deps.storage, &address)?;
    let available_amount = compute_available_amount(env.block.time.seconds(), &info)?;
    Ok(available_amount)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
