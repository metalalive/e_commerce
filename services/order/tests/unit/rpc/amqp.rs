use std::boxed::Box;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use tokio::task;
use tokio::time::sleep;

use ecommerce_common::confidentiality::UserSpaceConfidentiality;
use ecommerce_common::constant::env_vars::SYS_BASEPATH;
use ecommerce_common::error::AppErrorCode;

use order::error::AppError;
use order::{AbstractRpcContext, AppRpcClientReqProperty, AppSharedState};

use crate::ut_setup_share_state;

fn ut_appstate_setup() -> AppSharedState {
    let cfdntl = {
        let sys_basepath = env::var(SYS_BASEPATH).unwrap();
        let path = sys_basepath.clone() + "/common/data/secrets.json";
        UserSpaceConfidentiality::build(path)
    };
    ut_setup_share_state("config_ok_amqp.json", Box::new(cfdntl))
}

fn ut_client_publish_msgs() -> [(&'static str, &'static str); 5] {
    let routes = ["rpc.order.unittest.three", "rpc.order.unittest.two"];
    [
        (routes[0], r#"{"me":"je"}"#),
        (routes[0], r#"{"saya":"ich"}"#),
        (routes[1], r#"{"Zeist":"meat"}"#),
        (routes[0], r#"{"light":"shadow"}"#),
        (routes[1], r#"{"ice":"flame"}"#),
    ]
}
fn ut_server_publish_msg(req_content: &str) -> &'static str {
    match req_content {
        r#"{"me":"je"}"# => r#"{"devicetree":"ie80211_rx"}"#,
        r#"{"saya":"ich"}"# => r#"{"ext4_readdir":"inode"}"#,
        r#"{"Zeist":"meat"}"# => r#"{"kmem_cache_init":"sys_signal"}"#,
        r#"{"light":"shadow"}"# => r#"{"task_struct":"iirq_flgs"}"#,
        r#"{"ice":"flame"}"# => r#"{"vma_area":"do_pagefault"}"#,
        _others => r#"{"dev_null":"prng"}"#,
    }
}

async fn ut_client_send_req<'a>(
    rpcctx: Arc<Box<dyn AbstractRpcContext>>,
    route: &'a str,
    msg: &'a str,
) {
    let num_retry = 1;
    let result = rpcctx.as_ref().acquire(num_retry).await;
    assert!(result.is_ok());
    let hdlr = result.unwrap();
    let props = AppRpcClientReqProperty {
        msgbody: msg.as_bytes().to_vec(),
        start_time: Local::now().fixed_offset(),
        route: route.to_string(),
        correlation_id: None,
    };
    let result = hdlr.send_request(props).await;
    if let Err(e) = result.as_ref() {
        println!("[debug] client-send-request, error: {:?}", e);
    }
    assert!(result.is_ok());
    let mut event = result.unwrap();
    sleep(Duration::from_millis(40)).await;
    let mut possible_reply = None;
    for _ in 0..3 {
        match event.receive_response().await {
            Ok(reply) => {
                possible_reply = Some(reply);
                break;
            }
            Err(e) => {
                let result = matches!(e.code, AppErrorCode::RpcReplyNotReady);
                assert!(result);
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    assert!(possible_reply.is_some());
    let actual_content = possible_reply.unwrap().body;
    let actual_resp_body = String::from_utf8(actual_content).unwrap();
    println!("[debug] RPC reply content: {}", actual_resp_body);
    let expect_resp_body = ut_server_publish_msg(msg);
    assert_eq!(actual_resp_body.as_str(), expect_resp_body);
} // end of fn ut_client_send_req

fn mock_route_hdlr_wrapper(
    req: AppRpcClientReqProperty,
    _shr_state: AppSharedState,
) -> Pin<Box<dyn Future<Output = DefaultResult<Vec<u8>, AppError>> + Send>> {
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
async fn client_req_to_server_ok() {
    let shr_state = ut_appstate_setup();
    let rpcctx = shr_state.rpc();
    let rpc_srv = rpcctx.clone();
    let srv_handle = task::spawn(async move {
        // acquire server handler, declare/create queues at the beginning
        let result = rpc_srv
            .server_start(shr_state, mock_route_hdlr_wrapper)
            .await;
        assert!(result.is_ok());
    });
    sleep(Duration::from_secs(4)).await; // wait until queues are created
    let msgs = ut_client_publish_msgs();
    let mut clients_handle = Vec::new();
    for (route, msg) in msgs {
        let rpc_client = rpcctx.clone();
        let client_handle = task::spawn(async move {
            ut_client_send_req(rpc_client, route, msg).await;
        });
        clients_handle.insert(0, client_handle);
        sleep(Duration::from_millis(50)).await;
    }
    for client_handle in clients_handle {
        let result = client_handle.await;
        assert!(result.is_ok());
    }
    let result = srv_handle.await;
    assert!(result.is_ok());
} // end of fn client_req_to_server_ok
