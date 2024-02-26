use std::env;
use std::boxed::Box;
use std::sync::Arc;

use chrono::Local;
use order::{AppSharedState, AbsRpcClientCtx, AppRpcClientReqProperty};
use order::constant::ENV_VAR_SYS_BASE_PATH;
use order::confidentiality::UserSpaceConfidentiality;

use crate::ut_setup_share_state;


fn ut_appstate_setup() -> AppSharedState
{
    let cfdntl = {
        let sys_basepath = env::var(ENV_VAR_SYS_BASE_PATH).unwrap(); 
        let path = sys_basepath.clone() + "/common/data/secrets.json";
        UserSpaceConfidentiality::build(path)
    };
    ut_setup_share_state("config_ok_amqp.json", Box::new(cfdntl))
}

#[tokio::test]
async fn client_send_req_ok()
{
    let shr_state = ut_appstate_setup();
    let rpcctx = shr_state.rpc();
    let num_retry = 1;
    let msgs = [ r#"{"me":"je"}"#, r#"{"saya":"ich"}"#, r#"{"Zeist":"meat"}"#];
    for msg in msgs {
        let result = AbsRpcClientCtx::acquire(rpcctx.as_ref(), num_retry).await;
        assert!(result.is_ok());
        let hdlr = result.unwrap();
        let props = AppRpcClientReqProperty { retry: num_retry, msgbody: msg.as_bytes().to_vec() ,
            start_time: Local::now().fixed_offset(), route: "rpc.order.unittest.two".to_string()
        };
        let result = hdlr.send_request(props).await;
        // if let Err(e) = result.as_ref() {
        //     println!("[debug] error: {:?}", e);
        // }
        assert!(result.is_ok());
        let _event = result.unwrap();
        tokio::time::sleep( std::time::Duration::from_millis(15) ).await;
    }
}
