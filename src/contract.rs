#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:simple-option";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if msg.expires <= env.block.height {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: "Cannot create expired option".to_string(),
        }));
    }

    let state = State {
        creator: info.sender.clone(),
        owner: info.sender.clone(),
        collateral: info.funds,
        counter_offer: msg.counter_offer,
        expires: msg.expires,
    };

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient } => execute::transfer(deps, info, recipient),
        ExecuteMsg::Execute {} => execute::execute(deps, env, info),
        ExecuteMsg::Burn {} => execute::burn(deps, env, info),
    }
}

pub mod execute {
    use cosmwasm_std::{Addr, BankMsg};

    use super::*;

    pub fn transfer(
        deps: DepsMut,
        info: MessageInfo,
        recipient: Addr,
    ) -> Result<Response, ContractError> {
        let mut state = STATE.load(deps.storage)?;
        // ensure msg.sender is the owner
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }

        // set new owner on state
        state.owner = recipient.clone();
        STATE.save(deps.storage, &state)?;

        let response = Response::new()
            .add_attribute("action", "transfer")
            .add_attribute("owner", recipient.clone());
        Ok(response)
    }

    pub fn execute(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;
        // ensure msg.sender is the owner
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }

        // ensure not expired
        if env.block.height >= state.expires {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: "option expired".to_string(),
            }));
        }

        // ensure sending proper counter_offer
        if info.funds != state.counter_offer {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: format!("must send exact couter_offer: {:?}", state.counter_offer),
            }));
        }

        // release counter_offer to creator
        let res = Response::new()
            .add_message(BankMsg::Send {
                to_address: state.creator.into_string(),
                amount: state.counter_offer,
            })
            .add_message(BankMsg::Send {
                to_address: state.owner.into_string(),
                amount: state.collateral,
            })
            .add_attribute("action", "execute");

        STATE.remove(deps.storage);

        Ok(res)
    }

    pub fn burn(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;
        // ensure is expired
        if env.block.height < state.expires {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: "option expired".to_string(),
            }));
        }

        // ensure sending proper counter_offer
        if !info.funds.is_empty() {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: "Don't send funds with burn".to_string(),
            }));
        }

        // release collateral to creator
        let res = Response::new()
            .add_message(BankMsg::Send {
                to_address: state.creator.into_string(),
                amount: state.collateral,
            })
            .add_attribute("action", "burn");
        // delete the option
        STATE.remove(deps.storage);

        Ok(res)
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query::config(deps)?),
    }
}

pub mod query {
    use crate::msg::ConfigResponse;

    use super::*;

    pub fn config(deps: Deps) -> StdResult<ConfigResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
