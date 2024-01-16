#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
// use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};

use cw_controllers::Admin;

/*
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:assignment";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
*/

const ADMIN: Admin = Admin::new("admin");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // set_contract_version(_deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    ADMIN.set(deps.branch(), Some(info.sender))?;
    STATE.save(deps.storage, &State { deals: vec![] })?;
    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateDeal {
            seller,
            buyer,
            coin_a,
            coin_b,
            expiry,
        } => execute::create_deal(deps, env, info, seller, buyer, coin_a, coin_b, expiry),
        ExecuteMsg::Deposit {} => execute::deposit(deps, env, info),
        ExecuteMsg::CompleteDeal {} => execute::complete_deal(deps, env, info),
        ExecuteMsg::Withdraw {} => execute::withdraw(deps, env, info),
        ExecuteMsg::Reset {} => execute::reset(deps, env, info),
    }
}

pub mod execute {
    use crate::state::{Deal, Ics20Packet, State, STATE};
    use cosmwasm_std::{BankMsg, CosmosMsg, IbcMsg, StdError, Timestamp};

    use super::*;

    // Enables anyone to submit an OTC deal where coin_a belongs to seller and coin_b belongs to buyer.
    pub fn create_deal(
        deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        seller: String,
        buyer: String,
        coin_a: Coin,
        coin_b: Coin,
        expiry: u64,
    ) -> Result<Response, ContractError> {
        let deal = Deal {
            seller,
            buyer,
            coin_a,
            coin_b,
            expiry,
            finished: false,
            seller_deposited: false,
            buyer_deposited: false,
            seller_withdrew: false,
            buyer_withdrew: false,
            channel_id_recieved_a: "".to_string(),
            channel_id_recieved_b: "".to_string(),
        };

        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.deals.push(deal);
            Ok(state)
        })?;

        Ok(Response::new().add_attribute("method", "created_deal"))
    }

    // Enables the user on contract hosting chain to deposit the funds.
    pub fn deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
        let depositor = info.sender.clone().into_string();
        let state = STATE.load(deps.storage)?;

        // Find the index of deal matching with incoming deposit
        let deal_found = find_deal_with_denom(
            state.clone(),
            depositor.clone(),
            info.funds[0].denom.clone(),
        );

        // Get the deal if found
        if deal_found.is_some() {
            let deal_index = deal_found.unwrap();
            let deal = state.deals[deal_index].clone();

            // Deal expired and finished check
            deal_expired_or_finished(deal.clone(), env)?;

            // Deposit funds for seller
            if info.sender == deal.seller {
                STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
                    if state.deals[deal_index].coin_a.amount == info.funds[0].amount {
                        state.deals[deal_index].seller_deposited = true;
                    } else {
                        return Err(ContractError::Std(StdError::generic_err(
                            "Incorrect amount deposited",
                        )));
                    }

                    Ok(state)
                })?;
            // Deposit funds for buyer
            } else if info.sender == deal.buyer {
                STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
                    if state.deals[deal_index].coin_b.amount == info.funds[0].amount {
                        state.deals[deal_index].buyer_deposited = true;
                    } else {
                        return Err(ContractError::Std(StdError::generic_err(
                            "Incorrect amount deposited",
                        )));
                    }
                    Ok(state)
                })?;
            } else {
                return Err(ContractError::Unauthorized {});
            }
        } else {
            return Err(ContractError::Std(StdError::generic_err("No deal found")));
        }
        Ok(Response::new().add_attribute("method", "deposited"))
    }

    // Executed by user on contract hosting chain to complete the deal.
    pub fn complete_deal(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
    ) -> Result<Response, ContractError> {
        let withdrawer = info.sender.clone();
        let state = STATE.load(deps.storage)?;

        // Find the deal
        let deal_found_index = find_deal(state.clone(), withdrawer.clone().into_string());
        let deal_found = if deal_found_index.is_some() {
            Some(state.deals[deal_found_index.unwrap()].clone())
        } else {
            None
        };

        let deal_coin;
        let deal;
        let mut seller_withdrew = state.deals[deal_found_index.unwrap()].seller_withdrew;
        let mut buyer_withdrew = state.deals[deal_found_index.unwrap()].buyer_withdrew;

        if deal_found.is_some() {
            deal = deal_found.unwrap();

            deal_expired_or_finished(deal.clone(), env.clone())?;

            // Check if both parties have deposited
            if !deal.seller_deposited || !deal.buyer_deposited {
                return Err(ContractError::Std(StdError::generic_err(
                    "Both parties must deposit first",
                )));
            }

            deal_coin = if info.sender == deal.seller {
                seller_withdrew = true;

                deal.coin_b
            } else if info.sender == deal.buyer {
                buyer_withdrew = true;

                deal.coin_a
            } else {
                return Err(ContractError::Std(StdError::generic_err(
                    "No deposit found",
                )));
            };
        } else {
            return Err(ContractError::Std(StdError::generic_err("No deal found")));
        }

        let msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: withdrawer.into_string(),
            amount: vec![deal_coin],
        });

        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.deals[deal_found_index.unwrap()].seller_withdrew = seller_withdrew;
            state.deals[deal_found_index.unwrap()].buyer_withdrew = buyer_withdrew;

            // Mark deal as finished if both parties withdrew
            if seller_withdrew && buyer_withdrew {
                state.deals[deal_found_index.unwrap()].finished = true;
            }
            Ok(state)
        })?;

        Ok(Response::default().add_message(msg))
    }

    // Executed by user on contract hosting chain to withdraw the funds if deal is not completed.
    // Or other party has not deposited.
    pub fn withdraw(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
    ) -> Result<Response, ContractError> {
        let withdrawer = info.sender.clone();
        let state = STATE.load(deps.storage)?;

        // Find the deal
        let deal_found_index = find_deal(state.clone(), withdrawer.clone().into_string());
        let deal_found = if deal_found_index.is_some() {
            Some(state.deals[deal_found_index.unwrap()].clone())
        } else {
            None
        };

        let deal_coin;

        let mut seller_deposited = state.deals[deal_found_index.unwrap()].seller_deposited;
        let mut buyer_deposited = state.deals[deal_found_index.unwrap()].buyer_deposited;

        if deal_found.is_some() {
            let deal = deal_found.unwrap();

            if deal.finished {
                return Err(ContractError::Std(StdError::generic_err(
                    "Deal has already finished.",
                )));
            }

            // Don't allow withdrawal if both parties have deposited
            if deal.seller_deposited && deal.buyer_deposited {
                return Err(ContractError::Std(StdError::generic_err(
                    "Refund not allowed as both parties have deposited",
                )));
            }

            // If only one party has deposited, allow that party to withdraw if not already withdrawn
            // Mark party deposit as false
            deal_coin = if deal.seller_deposited && !deal.buyer_deposited {
                if withdrawer == deal.seller {
                    seller_deposited = false;
                    deal.coin_a
                } else {
                    return Err(ContractError::Unauthorized {});
                }
            } else if deal.buyer_deposited && !deal.seller_deposited {
                if withdrawer == deal.buyer {
                    buyer_deposited = false;
                    deal.coin_b
                } else {
                    return Err(ContractError::Unauthorized {});
                }
            } else {
                return Err(ContractError::Std(StdError::generic_err(
                    "No deposit found",
                )));
            };

            // Update deal with deposit status
            STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
                state.deals[deal_found_index.unwrap()].seller_deposited = seller_deposited;
                state.deals[deal_found_index.unwrap()].buyer_deposited = buyer_deposited;

                Ok(state)
            })?;
        } else {
            return Err(ContractError::Std(StdError::generic_err("No deal found")));
        }

        let msg = BankMsg::Send {
            to_address: withdrawer.into_string(),
            amount: vec![deal_coin],
        };

        Ok(Response::default().add_message(msg))
    }

    // Enables the admin to reset the deals for testing purposes.
    pub fn reset(deps: DepsMut, _env: Env, info: MessageInfo) -> Result<Response, ContractError> {
        let res = ADMIN.assert_admin(deps.as_ref(), &info.sender.clone()); // Check if admin

        if res.is_err() {
            return Err(ContractError::Unauthorized {});
        }

        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.deals = vec![];
            Ok(state)
        })?;

        Ok(Response::new().add_attribute("method", "reset"))
    }

    // Enabless the user on another chain to withdraw funds.
    pub fn withdraw_ibc(
        deps: DepsMut,
        env: Env,
        _channel: String,
        packet: Ics20Packet,
    ) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;
        let deal_found_index =
            find_deal_with_denom(state.clone(), packet.sender.clone(), packet.denom.clone());
        let deal_found = if deal_found_index.is_some() {
            Some(state.deals[deal_found_index.unwrap()].clone())
        } else {
            None
        };

        let withdrawer = packet.sender.clone();
        if deal_found.is_some() {
            let deal = deal_found.unwrap();

            if deal.finished {
                return Err(ContractError::Std(StdError::generic_err(
                    "Deal has already finished.",
                )));
            }

            // Don't allow withdrawal if both parties have deposited
            if deal.seller_deposited && deal.buyer_deposited {
                return Err(ContractError::Std(StdError::generic_err(
                    "Refund not allowed as both parties have deposited",
                )));
            }

            let deal_coin;
            let mut seller_deposited = state.deals[deal_found_index.unwrap()].seller_deposited;
            let mut buyer_deposited = state.deals[deal_found_index.unwrap()].buyer_deposited;
            let dest_channel;

            // If only one party has deposited, allow that party to withdraw if not already withdrawn
            // Mark party deposit as false
            deal_coin = if deal.seller_deposited && !deal.buyer_deposited {
                if withdrawer == deal.seller {
                    seller_deposited = false;
                    dest_channel = deal.channel_id_recieved_a;
                    deal.coin_a
                } else {
                    return Err(ContractError::Unauthorized {});
                }
            } else if deal.buyer_deposited && !deal.seller_deposited {
                if withdrawer == deal.buyer {
                    buyer_deposited = false;
                    dest_channel = deal.channel_id_recieved_b;
                    deal.coin_b
                } else {
                    return Err(ContractError::Unauthorized {});
                }
            } else {
                return Err(ContractError::Std(StdError::generic_err(
                    "No deposit found",
                )));
            };

            // Update deal with deposit status
            STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
                state.deals[deal_found_index.unwrap()].seller_deposited = seller_deposited;
                state.deals[deal_found_index.unwrap()].buyer_deposited = buyer_deposited;

                Ok(state)
            })?;

            let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
                channel_id: dest_channel,
                to_address: withdrawer.to_string(),
                amount: deal_coin,
                timeout: env.block.time.plus_seconds(100).into(),
            });

            Ok(Response::default().add_message(msg))
        } else {
            return Err(ContractError::Std(StdError::generic_err("No deal found")));
        }
    }

    // Enables the user on another chain to deposit funds.
    pub fn deposit_ibc(
        deps: DepsMut,
        env: Env,
        channel: String,
        packet: Ics20Packet,
    ) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;

        let deal_found_index =
            find_deal_with_denom(state.clone(), packet.sender.clone(), packet.denom.clone());
        let deal_found = if deal_found_index.is_some() {
            Some(state.deals[deal_found_index.unwrap()].clone())
        } else {
            None
        };

        if deal_found.is_some() {
            let deal = deal_found.unwrap();

            deal_expired_or_finished(deal.clone(), env.clone())?;

            // Deposit funds for seller
            if packet.sender == deal.seller {
                STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
                    if state.deals[deal_found_index.unwrap()].coin_a.amount == packet.amount {
                        state.deals[deal_found_index.unwrap()].seller_deposited = true;
                        state.deals[deal_found_index.unwrap()].channel_id_recieved_a = channel;
                    } else {
                        return Err(ContractError::Std(StdError::generic_err(
                            "Incorrect amount deposited",
                        )));
                    }

                    Ok(state)
                })?;
            // Deposit funds for buyer
            } else if packet.sender == deal.buyer {
                STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
                    if state.deals[deal_found_index.unwrap()].coin_b.amount == packet.amount {
                        state.deals[deal_found_index.unwrap()].buyer_deposited = true;
                        state.deals[deal_found_index.unwrap()].channel_id_recieved_b = channel;
                    } else {
                        return Err(ContractError::Std(StdError::generic_err(
                            "Incorrect amount deposited",
                        )));
                    }
                    Ok(state)
                })?;
            } else {
                return Err(ContractError::Unauthorized {});
            }
        } else {
            return Err(ContractError::Std(StdError::generic_err("No deal found")));
        }
        Ok(Response::default())
    }

    // Executed by user on another chain to complete the deal.
    pub fn deal_complete_ibc(
        deps: DepsMut,
        env: Env,
        _channel: String,
        packet: Ics20Packet,
    ) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;
        let withdrawer = packet.sender.clone();

        // Find the deal
        let deal_found_index = find_deal(state.clone(), withdrawer.clone());
        let deal_found = if deal_found_index.is_some() {
            Some(state.deals[deal_found_index.unwrap()].clone())
        } else {
            None
        };

        let dest_channel;
        let deal_coin;
        let deal;
        let mut seller_withdrew = state.deals[deal_found_index.unwrap()].seller_withdrew;
        let mut buyer_withdrew = state.deals[deal_found_index.unwrap()].buyer_withdrew;

        if deal_found.is_some() {
            deal = deal_found.unwrap();

            deal_expired_or_finished(deal.clone(), env.clone())?;

            // Check if both parties have deposited
            if !deal.seller_deposited || !deal.buyer_deposited {
                return Err(ContractError::Std(StdError::generic_err(
                    "Both parties must deposit first",
                )));
            }

            deal_coin = if withdrawer == deal.seller {
                seller_withdrew = true;
                dest_channel = deal.channel_id_recieved_b;
                deal.coin_b
            } else if withdrawer == deal.buyer {
                buyer_withdrew = true;
                dest_channel = deal.channel_id_recieved_a;
                deal.coin_a
            } else {
                return Err(ContractError::Std(StdError::generic_err(
                    "No deposit found",
                )));
            };
        } else {
            return Err(ContractError::Std(StdError::generic_err("No deal found")));
        }

        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.deals[deal_found_index.unwrap()].seller_withdrew = seller_withdrew;
            state.deals[deal_found_index.unwrap()].buyer_withdrew = buyer_withdrew;

            // Mark deal as finished if both parties withdrew
            if seller_withdrew && buyer_withdrew {
                state.deals[deal_found_index.unwrap()].finished = true;
            }
            Ok(state)
        })?;

        let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
            channel_id: dest_channel,
            to_address: withdrawer.to_string(),
            amount: deal_coin,
            timeout: env.block.time.plus_seconds(100).into(),
        });

        Ok(Response::default().add_message(msg))
    }

    // Enables the admin to change the expiry of the deal.
    pub fn change_expiry(
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        expiry: u64,
        deal_id: u64,
    ) -> Result<Response, ContractError> {
        let res = ADMIN.assert_admin(deps.as_ref(), &info.sender.clone()); // Check if admin

        if res.is_err() {
            return Err(ContractError::Unauthorized {});
        }

        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.deals[deal_id as usize].expiry = expiry;
            Ok(state)
        })?;

        Ok(Response::new().add_attribute("method", "changed_expiry"))
    }

    // Find deal by seller or buyer
    fn find_deal(state: State, sender: String) -> Option<usize> {
        for (i, deal) in state.deals.iter().enumerate() {
            if deal.seller == sender || deal.buyer == sender {
                return Some(i);
            }
        }
        None
    }

    // Find deal by seller or buyer and denom
    fn find_deal_with_denom(state: State, sender: String, denom: String) -> Option<usize> {
        for (i, deal) in state.deals.iter().enumerate() {
            if (deal.seller == sender && deal.coin_a.denom == denom)
                || (deal.buyer == sender && deal.coin_b.denom == denom)
            {
                return Some(i);
            }
        }
        None
    }

    // Check if deal has expired or finished
    fn deal_expired_or_finished(deal: Deal, env: Env) -> Result<(), ContractError> {
        let expiry = Timestamp::from_seconds(deal.expiry);
        if env.block.time > expiry || deal.finished {
            return Err(ContractError::Std(StdError::generic_err(
                "Deal has expired or already finished.",
            )));
        }
        Ok(())
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBalances { address } => to_json_binary(&query::get_balances(deps, address)?),
        QueryMsg::GetDeal { id } => to_json_binary(&query::get_deal(deps, id)?),
    }
}

mod query {
    use cosmwasm_std::{to_json_binary, Binary, Deps, StdResult};

    pub fn get_balances(deps: Deps, address: String) -> StdResult<Binary> {
        let balances = deps
            .querier
            .query_all_balances(address.to_string())
            .unwrap();
        to_json_binary(&balances)
    }

    pub fn get_deal(deps: Deps, id: u64) -> StdResult<Binary> {
        let state = super::STATE.load(deps.storage)?;
        let deal = state.deals[id as usize].clone();
        to_json_binary(&deal)
    }
}
