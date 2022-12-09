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
                msg: "option not yet expired".to_string(),
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
    use crate::contract::execute::{burn, execute, transfer};
    use crate::msg::ConfigResponse;

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr, Attribute, BankMsg, CosmosMsg};

    #[test]
    fn test_proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            counter_offer: coins(40, "ETH"),
            expires: 100_000,
        };
        let info = mock_info("creator", &coins(1, "BTC"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(100_000, value.expires);
        assert_eq!("creator", value.owner.into_string());
        assert_eq!("creator", value.creator.into_string());
        assert_eq!(coins(1, "BTC"), value.collateral);
        assert_eq!(coins(40, "ETH"), value.counter_offer);
    }

    #[test]
    fn test_transfer() {
        let msg = InstantiateMsg {
            counter_offer: coins(40, "ETH"),
            expires: 100_000,
        };
        let info = mock_info("creator", &coins(1, "BTC"));
        let mut deps = mock_dependencies();

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        // random cannot transfer
        let info = mock_info("anyone", &[]);
        let err = transfer(deps.as_mut(), info, Addr::unchecked("anyone")).unwrap_err();
        match err {
            ContractError::Unauthorized { .. } => {}
            e => panic!("unexpected error: {}", e),
        }

        // owner can transfer
        let info = mock_info("creator", &[]);
        let res = transfer(deps.as_mut(), info, Addr::unchecked("someone")).unwrap();
        assert_eq!(res.attributes.len(), 2);
        assert_eq!(
            res.attributes[0],
            Attribute {
                key: "action".to_string(),
                value: "transfer".to_string()
            }
        );

        // check updated properly
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("someone", value.owner.as_str());
        assert_eq!("creator", value.creator.as_str());
    }

    #[test]
    fn test_execute() {
        let mut deps = mock_dependencies();

        let counter_offer = coins(40, "ETH");
        let collateral = coins(1, "BTC");

        let msg = InstantiateMsg {
            counter_offer: counter_offer.clone(),
            expires: 100_000,
        };
        let info = mock_info("creator", &collateral);

        let _ = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        let _ = transfer(deps.as_mut(), info, Addr::unchecked("owner")).unwrap();

        // random cannot execute
        let info = mock_info("anyone", &counter_offer);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info).unwrap_err();
        match err {
            ContractError::Unauthorized { .. } => {}
            e => panic!("unexpected error: {}", e),
        }

        // expired cannot execute
        let info = mock_info("owner", &counter_offer);
        let mut env = mock_env();
        env.block.height = 200_000;
        let err = execute(deps.as_mut(), env, info).unwrap_err();
        match err {
            ContractError::Std(StdError::GenericErr { msg }) => {
                assert_eq!(msg.as_str(), "option expired")
            }
            e => panic!("unexpected error: {}", e),
        }

        // expired cannot execute
        let info = mock_info("owner", &coins(39, "ETH"));
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info).unwrap_err();
        match err {
            ContractError::Std(StdError::GenericErr { msg }) => {
                assert!(msg.contains("must send exact couter_offer"))
            }
            e => panic!("unexpected error: {}", e),
        }

        // proper execution
        let info = mock_info("owner", &counter_offer);
        let env = mock_env();
        let res = execute(deps.as_mut(), env, info).unwrap();
        assert_eq!(res.messages.len(), 2);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".into(),
                amount: counter_offer.clone()
            })
        );
        assert_eq!(
            res.messages[1].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "owner".into(),
                amount: collateral.clone()
            })
        );

        // check deleted
        let _ = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap_err();
    }

    #[test]
    fn test_burn() {
        let mut deps = mock_dependencies();

        let counter_offer = coins(40, "ETH");
        let collateral = coins(1, "BTC");

        let msg = InstantiateMsg {
            counter_offer: counter_offer.clone(),
            expires: 100_000,
        };
        let info = mock_info("creator", &collateral);

        let _ = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &[]);
        let _ = transfer(deps.as_mut(), info, Addr::unchecked("owner")).unwrap();

        // non-expired cannot execute
        let info = mock_info("anyone", &[]);
        let env = mock_env();
        let err = burn(deps.as_mut(), env, info).unwrap_err();
        match err {
            ContractError::Std(StdError::GenericErr { msg }) => {
                assert_eq!(msg.as_str(), "option not yet expired")
            }
            e => panic!("unexpected error: {}", e),
        }

        // with funds cannot execute
        let info = mock_info("anyone", &counter_offer);
        let mut env = mock_env();
        env.block.height = 200_000;
        let err = burn(deps.as_mut(), env, info).unwrap_err();
        match err {
            ContractError::Std(StdError::GenericErr { msg }) => {
                assert_eq!(msg.as_str(), "Don't send funds with burn")
            }
            e => panic!("unexpected error: {}", e),
        }

        // proper execution
        let info = mock_info("anyone", &[]);
        let mut env = mock_env();
        env.block.height = 200_000;
        let res = burn(deps.as_mut(), env, info).unwrap();
        assert_eq!(res.messages.len(), 1);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".into(),
                amount: collateral.clone()
            })
        );

        // check deleted
        let _ = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap_err();
    }
}
