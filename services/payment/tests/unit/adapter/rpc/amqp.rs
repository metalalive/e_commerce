use std::boxed::Box;
use std::sync::Arc;
use std::time::Duration;

use actix_web::rt;
use futures_util::StreamExt;
use lapin::options::{BasicConsumeOptions, BasicPublishOptions};
use lapin::protocol::basic::AMQPProperties;
use lapin::types::FieldTable;
use lapin::uri::{AMQPAuthority, AMQPQueryString, AMQPScheme, AMQPUri, AMQPUserInfo};
use lapin::{Channel, Connection, ConnectionProperties, Consumer};
use serde::Deserialize;

use ecommerce_common::confidentiality::{self, AbstractConfidentiality};
use ecommerce_common::config::{AppAmqpBindingCfg, AppConfig, AppRpcAmqpCfg, AppRpcCfg};
use payment::adapter::rpc::{AbstractRpcContext, AppRpcClientRequest, AppRpcCtxError};

use super::ut_clone_amqp_binding_cfg;
use crate::ut_setup_sharestate;

#[allow(clippy::upper_case_acronyms)]
#[derive(Deserialize)]
struct SECRET {
    host: String,
    port: u16,
    username: String,
    password: String,
}

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
    let mut evt = result?;
    let expect_reply_msgs = ut_server_publish_msg(msg).to_string().into_bytes();
    let actual_reply_msgs = evt.receive_response().await?.message;
    assert_eq!(actual_reply_msgs, expect_reply_msgs);
    // assert!(false);
    Ok(())
} // end of fn ut_client_send_req

async fn ut_setup_mockserver_conn(
    cfdntl: Box<dyn AbstractConfidentiality>,
    rpccfg: &AppRpcAmqpCfg,
) -> Connection {
    let confidential_path = rpccfg.confidential_id.as_str();
    let serial = cfdntl.try_get_payload(confidential_path).unwrap();
    let SECRET {
        host,
        port,
        username,
        password,
    } = serde_json::from_str::<SECRET>(serial.as_str()).unwrap();
    let uri = AMQPUri {
        scheme: AMQPScheme::AMQP,
        authority: AMQPAuthority {
            host,
            port,
            userinfo: AMQPUserInfo { username, password },
        },
        vhost: rpccfg.attributes.vhost.clone(),
        query: AMQPQueryString::default(),
    };
    let options = ConnectionProperties::default();
    Connection::connect_uri(uri, options).await.unwrap()
} // end of fn ut_setup_mockserver_conn

async fn ut_server_start_consume(
    channel: Channel,
    mut consumer: Consumer,
    bindcfg: AppAmqpBindingCfg,
    expect_nummsgs_recv: usize,
) -> Result<(), String> {
    let orig_publisher_msgs = ut_client_publish_msgs();
    let expect_routekey = bindcfg.routing_key.as_str();
    let mut actual_nummsgs_recv = 0usize;
    for _ in 0..expect_nummsgs_recv {
        // ---------------------
        let r = if let Some(r2) = consumer.next().await {
            r2
        } else {
            break;
        };
        let deliver = r.map_err(|e| e.to_string())?;
        actual_nummsgs_recv += 1;
        let actual_routekey = deliver.routing_key.as_str();
        assert_eq!(actual_routekey, expect_routekey);
        let (actual_msg, props) = (deliver.data, deliver.properties);
        let result = orig_publisher_msgs
            .iter()
            .find(|v| v.1 == actual_routekey && v.2.to_string().into_bytes() == actual_msg);
        assert!(result.is_some());
        // ---------------------
        let expect_reply_msgs = ut_server_publish_msg(result.unwrap().2)
            .to_string()
            .into_bytes();
        let reply_to = props
            .reply_to()
            .as_ref()
            .ok_or("utest-missing-reply-to".to_string())?;
        let corr_id = props
            .correlation_id()
            .as_ref()
            .ok_or("utest-missing-corr-id".to_string())?;
        // println!("[debug] server-recv-request, reply-to: {:?}", reply_to);
        let properties = AMQPProperties::default()
            .with_correlation_id(corr_id.as_str().into())
            .with_content_encoding("utf-8".into())
            .with_content_type("application/json".into())
            .with_delivery_mode(if bindcfg.durable { 2 } else { 1 });
        let _confirm = channel
            .basic_publish(
                "", // implicitly apply anonymous exchange
                reply_to.as_str(),
                BasicPublishOptions {
                    mandatory: true,
                    immediate: false,
                },
                &expect_reply_msgs,
                properties,
            )
            .await
            .unwrap()
            .await
            .unwrap();
    } // end of loop
    assert_eq!(expect_nummsgs_recv, actual_nummsgs_recv);
    Ok(())
} // end of fn ut_server_start_consume

async fn ut_mock_server_start(app_cfg: Arc<AppConfig>) -> Result<(), String> {
    let cfdntl = confidentiality::build_context(app_cfg.as_ref())
        .map_err(|_e| "unit-test credential error".to_string())?;
    let rpccfg = if let AppRpcCfg::AMQP(c) = &app_cfg.api_server.rpc {
        c
    } else {
        return Err("unit-test cfg error".to_string());
    };
    let conn = ut_setup_mockserver_conn(cfdntl, &rpccfg).await;
    let chn = conn.create_channel().await.map_err(|e| e.to_string())?;
    let options = BasicConsumeOptions {
        no_local: false,
        no_ack: true,
        exclusive: false,
        nowait: false,
    };
    let mut handles = Vec::new();
    for bindcfg in rpccfg.bindings.iter() {
        let consumer_tag = format!("unit-test-server-{}", bindcfg.queue.as_str());
        let consumer = chn
            .basic_consume(
                bindcfg.queue.as_str(),
                consumer_tag.as_str(),
                options.clone(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| e.to_string())?;
        let c_fut = ut_server_start_consume(
            chn.clone(),
            consumer,
            ut_clone_amqp_binding_cfg(bindcfg),
            5usize,
        );
        let handle = rt::spawn(c_fut);
        handles.push(handle);
    } // end of loop
    assert!(!handles.is_empty());
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok());
    }
    // assert!(false);
    let _result = conn.close(0, "unit-test-mock-server").await;
    Ok(())
} // end of fn ut_mock_server_start

#[actix_web::test]
async fn client_req_to_server_ok() {
    let shr_state = ut_setup_sharestate();
    let rpcctx = shr_state.rpc_context();
    let mock_srv_handle = rt::spawn(ut_mock_server_start(shr_state.config()));
    rt::time::sleep(Duration::from_secs(4)).await;
    let msgs = ut_client_publish_msgs();
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
    let result = mock_srv_handle.await;
    assert!(result.is_ok());
} // end of fn client_req_to_server_ok
