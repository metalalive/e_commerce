use std::boxed::Box;
use std::sync::Arc;
use std::time::Duration;

use actix_web::rt;
use payment::adapter::rpc::{
    AbsRpcClientContext, AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError,
};

use crate::ut_setup_sharestate;

fn ut_client_publish_msgs() -> [(&'static str, &'static str, &'static str); 5] {
    let routes = ["rpc.payment.unittest.one"];
    [
        ("uzbek", routes[0], r#"{"me":"je"}"#),
        ("nippon", routes[0], r#"{"saya":"ich"}"#),
        ("jawa", routes[0], r#"{"Zeist":"meat"}"#),
        ("azajt", routes[0], r#"{"light":"shadow"}"#),
        ("cayman", routes[0], r#"{"ice":"flame"}"#),
    ]
}

async fn ut_client_send_req<'a>(
    rpcctx: Arc<Box<dyn AbstractRpcContext>>,
    req_id: &'a str,
    route: &'a str,
    msg: &'a str,
) -> Result<(), AppRpcCtxError> {
    let result = rpcctx.acquire().await;
    assert!(result.is_ok());
    let hdlr = result?;
    let props = AppRpcClientRequest {
        id: req_id.to_string(),
        message: msg.as_bytes().to_vec(),
        route: route.to_string(),
    };
    let result = hdlr.send_request(props).await;
    // if let Err(e) = result.as_ref() {
    //     println!("[debug] client-send-request, error: {:?}", e);
    // }
    assert!(result.is_ok());
    let _evt = result?;
    // TODO, waiting for reply
    Ok(())
}

#[actix_web::test]
async fn client_req_to_server_ok() {
    let shr_state = ut_setup_sharestate();
    let rpcctx = shr_state.rpc_context();
    let msgs = ut_client_publish_msgs();
    // TODO, build consumer for test
    let mut clients_handle = Vec::new();
    for (req_id, route, msg) in msgs {
        let rpc_client = rpcctx.clone();
        let client_handle = rt::spawn(ut_client_send_req(rpc_client, req_id, route, msg));
        clients_handle.insert(0, client_handle);
        rt::time::sleep(Duration::from_millis(50)).await;
    }
    for client_handle in clients_handle {
        let result = client_handle.await;
        assert!(result.is_ok());
    }
}
