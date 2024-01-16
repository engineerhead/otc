#[cfg(test)]
mod tests {
    use crate::helpers::CwTemplateContract;
    use crate::msg::InstantiateMsg;
    use cosmwasm_std::{Addr, Coin, Empty, Uint128};
    use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};

    pub fn contract_template() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        );
        Box::new(contract)
    }

    const SELLER: &str = "terra1qnuxey4pn4frkc2e9r6shhtkw2f8tkuwwualnn";
    const BUYER: &str = "terra1qnuxey4pn4frkc2e9r6shhtkw2f8tkuwwualnm";
    const USER: &str = "USER";
    const ADMIN: &str = "ADMIN";
    const NATIVE_DENOM1: &str = "denom1";
    const NATIVE_DENOM2: &str = "denom2";

    fn mock_app() -> App {
        AppBuilder::new().build(|router, _, storage| {
            router
                .bank
                .init_balance(
                    storage,
                    &Addr::unchecked(SELLER),
                    vec![Coin {
                        denom: NATIVE_DENOM1.to_string(),
                        amount: Uint128::new(1000),
                    }],
                )
                .unwrap();

            router
                .bank
                .init_balance(
                    storage,
                    &Addr::unchecked(BUYER),
                    vec![Coin {
                        denom: NATIVE_DENOM2.to_string(),
                        amount: Uint128::new(500),
                    }],
                )
                .unwrap();
        })
    }

    fn proper_instantiate() -> (App, CwTemplateContract) {
        let mut app = mock_app();
        let cw_template_id = app.store_code(contract_template());

        let msg = InstantiateMsg {};
        let cw_template_contract_addr = app
            .instantiate_contract(
                cw_template_id,
                Addr::unchecked(ADMIN),
                &msg,
                &[],
                "test",
                None,
            )
            .unwrap();

        let cw_template_contract = CwTemplateContract(cw_template_contract_addr);

        (app, cw_template_contract)
    }

    mod assignment {
        use super::*;
        use crate::msg::ExecuteMsg;

        #[test]
        fn deal_succeeds() {
            let (mut app, cw_template_contract) = proper_instantiate();

            let msg = ExecuteMsg::CreateDeal {
                seller: SELLER.to_string(),
                buyer: BUYER.to_string(),
                coin_a: Coin {
                    denom: NATIVE_DENOM1.to_string(),
                    amount: Uint128::new(100),
                },
                coin_b: Coin {
                    denom: NATIVE_DENOM2.to_string(),
                    amount: Uint128::new(200),
                },
                expiry: 1704400324,
            };
            let cosmos_msg = cw_template_contract.call(msg).unwrap();
            app.execute(Addr::unchecked(USER), cosmos_msg).unwrap();

            let msg = ExecuteMsg::Deposit {};

            app.execute_contract(
                Addr::unchecked(SELLER),
                cw_template_contract.addr(),
                &msg,
                &[Coin::new(100u128, NATIVE_DENOM1)],
            )
            .unwrap();

            app.execute_contract(
                Addr::unchecked(BUYER),
                cw_template_contract.addr(),
                &msg,
                &[Coin::new(200u128, NATIVE_DENOM2)],
            )
            .unwrap();

            // Complete deal
            let msg = ExecuteMsg::CompleteDeal {};
            app.execute_contract(
                Addr::unchecked(SELLER),
                cw_template_contract.addr(),
                &msg,
                &[],
            )
            .unwrap();

            app.execute_contract(
                Addr::unchecked(BUYER),
                cw_template_contract.addr(),
                &msg,
                &[],
            )
            .unwrap();

            // Get balances
            let balance_seller = app
                .wrap()
                .query_all_balances(Addr::unchecked(SELLER))
                .unwrap();
            let balance_buyer = app
                .wrap()
                .query_all_balances(Addr::unchecked(BUYER))
                .unwrap();

            assert_eq!(
                balance_seller,
                vec![
                    Coin::new(900u128, NATIVE_DENOM1),
                    Coin::new(200u128, NATIVE_DENOM2)
                ]
            );

            assert_eq!(
                balance_buyer,
                vec![
                    Coin::new(100u128, NATIVE_DENOM1),
                    Coin::new(300u128, NATIVE_DENOM2)
                ]
            );
        }

        #[test]
        fn withdrawal_fails_if_no_deposits() {
            let (mut app, cw_template_contract) = proper_instantiate();

            let msg = ExecuteMsg::CreateDeal {
                seller: SELLER.to_string(),
                buyer: BUYER.to_string(),
                coin_a: Coin {
                    denom: NATIVE_DENOM1.to_string(),
                    amount: Uint128::new(100),
                },
                coin_b: Coin {
                    denom: NATIVE_DENOM2.to_string(),
                    amount: Uint128::new(200),
                },
                expiry: 1704400324,
            };
            let cosmos_msg = cw_template_contract.call(msg).unwrap();
            app.execute(Addr::unchecked(USER), cosmos_msg).unwrap();

            let msg = ExecuteMsg::Withdraw {};

            let res = app.execute_contract(
                Addr::unchecked(SELLER),
                cw_template_contract.addr(),
                &msg,
                &[],
            );

            // Withdraw before deposit should fail
            assert_eq!(res.is_err(), true);
        }

        #[test]
        fn withdrawal_fails_if_both_deposit() {
            let (mut app, cw_template_contract) = proper_instantiate();

            let msg = ExecuteMsg::CreateDeal {
                seller: SELLER.to_string(),
                buyer: BUYER.to_string(),
                coin_a: Coin {
                    denom: NATIVE_DENOM1.to_string(),
                    amount: Uint128::new(100),
                },
                coin_b: Coin {
                    denom: NATIVE_DENOM2.to_string(),
                    amount: Uint128::new(200),
                },
                expiry: 1704400324,
            };
            let cosmos_msg = cw_template_contract.call(msg).unwrap();
            app.execute(Addr::unchecked(USER), cosmos_msg).unwrap();

            let msg = ExecuteMsg::Deposit {};
            app.execute_contract(
                Addr::unchecked(SELLER),
                cw_template_contract.addr(),
                &msg,
                &[Coin::new(100u128, NATIVE_DENOM1)],
            )
            .unwrap();

            app.execute_contract(
                Addr::unchecked(BUYER),
                cw_template_contract.addr(),
                &msg,
                &[Coin::new(200u128, NATIVE_DENOM2)],
            )
            .unwrap();

            let msg = ExecuteMsg::Withdraw {};

            let res = app.execute_contract(
                Addr::unchecked(SELLER),
                cw_template_contract.addr(),
                &msg,
                &[],
            );

            // Withdraw after both deposits should also fail
            assert_eq!(res.is_err(), true);
        }

        #[test]
        fn withdrawal_succeeds_if_one_party_deposits() {
            let (mut app, cw_template_contract) = proper_instantiate();

            let msg = ExecuteMsg::CreateDeal {
                seller: SELLER.to_string(),
                buyer: BUYER.to_string(),
                coin_a: Coin {
                    denom: NATIVE_DENOM1.to_string(),
                    amount: Uint128::new(100),
                },
                coin_b: Coin {
                    denom: NATIVE_DENOM2.to_string(),
                    amount: Uint128::new(200),
                },
                expiry: 1704400324,
            };
            let cosmos_msg = cw_template_contract.call(msg).unwrap();
            app.execute(Addr::unchecked(USER), cosmos_msg).unwrap();

            let msg = ExecuteMsg::Deposit {};
            app.execute_contract(
                Addr::unchecked(SELLER),
                cw_template_contract.addr(),
                &msg,
                &[Coin::new(100u128, NATIVE_DENOM1)],
            )
            .unwrap();

            let msg = ExecuteMsg::Withdraw {};

            let res = app.execute_contract(
                Addr::unchecked(SELLER),
                cw_template_contract.addr(),
                &msg,
                &[],
            );

            // Withdrawal after one party deposits, should succeed
            assert_eq!(res.is_ok(), true);

            let msg = ExecuteMsg::Deposit {};
            app.execute_contract(
                Addr::unchecked(BUYER),
                cw_template_contract.addr(),
                &msg,
                &[Coin::new(200u128, NATIVE_DENOM2)],
            )
            .unwrap();

            let msg = ExecuteMsg::Withdraw {};

            let res = app.execute_contract(
                Addr::unchecked(BUYER),
                cw_template_contract.addr(),
                &msg,
                &[],
            );

            assert_eq!(res.is_ok(), true);
        }
    }
}
