use std::boxed::Box;
use std::sync::Arc;
use std::time::Duration;

use actix_web::rt;
use payment::adapter::rpc::{AbsRpcClientContext, AbstractRpcContext};

use crate::ut_setup_sharestate;

fn ut_client_publish_msgs() -> [(&'static str, &'static str); 5] {
    let routes = ["rpc.order.unittest.one"];
    [
        (routes[0], r#"{"me":"je"}"#),
        (routes[0], r#"{"saya":"ich"}"#),
        (routes[0], r#"{"Zeist":"meat"}"#),
        (routes[0], r#"{"light":"shadow"}"#),
        (routes[0], r#"{"ice":"flame"}"#),
    ]
}

async fn ut_client_send_req<'a>(
    rpcctx: Arc<Box<dyn AbstractRpcContext>>,
    _route: &'a str,
    _msg: &'a str,
) {
    let result = rpcctx.acquire().await;
    assert!(result.is_ok());
}

#[actix_web::test]
async fn client_req_to_server_ok() {
    let shr_state = ut_setup_sharestate();
    let rpcctx = shr_state.rpc_context();
    let msgs = ut_client_publish_msgs();
    let mut clients_handle = Vec::new();
    for (route, msg) in msgs {
        let rpc_client = rpcctx.clone();
        let client_handle = rt::spawn(async move {
            ut_client_send_req(rpc_client, route, msg).await;
        });
        clients_handle.insert(0, client_handle);
        rt::time::sleep(Duration::from_millis(50)).await;
    }
    for client_handle in clients_handle {
        let result = client_handle.await;
        assert!(result.is_ok());
    }
}
