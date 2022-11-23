use crate::contract::{instantiate, query};

use crate::msg::{ConfigResponse, InstantiateMsg, QueryMsg};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_binary, Addr};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        token_addr: "vested_token".to_string(),
    };

    let env = mock_env();
    let info = mock_info("addr1234", &vec![]);
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<ConfigResponse>(&query(deps.as_ref(), env, QueryMsg::Config {}).unwrap())
            .unwrap(),
        ConfigResponse {
            owner: Addr::unchecked("owner"),
            token_addr: Addr::unchecked("vested_token"),
        }
    );
}
