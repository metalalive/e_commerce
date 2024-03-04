use std::env;
use std::boxed::Box;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::result::Result as DefaultResult;
use std::time::Duration;

use order::error::AppError;
use tokio::task;
use tokio::time::sleep;
use chrono::Local;

use order::{AppSharedState, AppRpcClientReqProperty, AbstractRpcContext};
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

fn ut_client_publish_msgs() -> [(&'static str, &'static str);5]
{
    let routes = ["rpc.order.unittest.three", "rpc.order.unittest.two"];
    [
        (routes[0],  r#"{"me":"je"}"# ), 
        (routes[0],  r#"{"saya":"ich"}"# ),
        (routes[1],  r#"{"Zeist":"meat"}"#),
        (routes[0],  r#"{"light":"shadow"}"#),
        (routes[1],  r#"{"ice":"flame"}"#),
    ]
}
fn ut_server_publish_msg(req_content:&str) -> &'static str
{
    match req_content {
        r#"{"me":"je"}"#     => r#"{"devicetree":"ie80211_rx"}"#,
        r#"{"saya":"ich"}"#  => r#"{"ext4_readdir":"inode"}"#,
        r#"{"Zeist":"meat"}"#    => r#"{"kmem_cache_init":"sys_signal"}"#,
        r#"{"light":"shadow"}"#  => r#"{"task_struct":"iirq_flgs"}"#,
        r#"{"ice":"flame"}"#     => r#"{"vma_area":"do_pagefault"}"#,
        _others => r#"{"dev_null":"prng"}"#,
    }
}

async fn ut_client_send_req<'a>(
    rpcctx: Arc<Box<dyn AbstractRpcContext>>, route:&'a str, msg: &'a str
)
{
    let num_retry = 1;
    let result = rpcctx.as_ref().acquire(num_retry).await;
    assert!(result.is_ok());
    let hdlr = result.unwrap();
    let props = AppRpcClientReqProperty { retry: num_retry, msgbody: msg.as_bytes().to_vec() ,
        start_time: Local::now().fixed_offset(), route: route.to_string()
    };
    let result = hdlr.send_request(props).await;
    //if let Err(e) = result.as_ref() {
    //    println!("[debug] error: {:?}", e);
    //}
    assert!(result.is_ok());
    let _event = result.unwrap();
    sleep(Duration::from_millis(15)).await;
}


fn mock_route_hdlr_wrapper(req:AppRpcClientReqProperty, shr_state: AppSharedState)
    -> Pin<Box<dyn Future<Output=DefaultResult<Vec<u8>, AppError>> + Send>>
{
    let expect_msgs = ut_client_publish_msgs();
    let fut = async move {
        let (route, content) = (req.route, String::from_utf8(req.msgbody).unwrap());
        let actual = (route.as_str(), content.as_str());
        let exist = expect_msgs.contains(&actual);
        assert!(exist);
        let resp_body = ut_server_publish_msg(content.as_str());
        Ok(resp_body.as_bytes().to_vec())
    };
    Box::pin(fut)
}

#[tokio::test]
async fn client_req_to_server_ok()
{
    let shr_state = ut_appstate_setup();
    let rpcctx = shr_state.rpc();
    let rpc_srv = rpcctx.clone();
    let srv_handle = task::spawn(async move {
        // acquire server handler, declare/create queues at the beginning
        let result = rpc_srv.server_start(shr_state, mock_route_hdlr_wrapper).await;
        assert!(result.is_ok());
    });
    sleep(Duration::from_secs(4)).await; // wait until queues are created
    let msgs = ut_client_publish_msgs();
    for (route, msg) in msgs {
        ut_client_send_req(rpcctx.clone(), route, msg).await;
    }
    let result = srv_handle.await;
    assert!(result.is_ok());
} // end of fn client_req_to_server_ok
