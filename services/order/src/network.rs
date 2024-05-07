use std::io::ErrorKind;
use std::net::ToSocketAddrs;
use std::result::Result as DefaultResult;

use axum::Router;
use http_body::Body as HttpBody;
use hyper::server::conn::AddrIncoming;
use hyper::server::Builder as HyperSrvBuilder;
use hyper::server::Server as HyperServer;

use ecommerce_common::error::AppErrorCode;

use crate::api::web::{ApiRouteTableType, ApiRouteType};
use crate::error::AppError;
use crate::{AppSharedState, WebApiListenCfg, WebApiRouteCfg};

pub type WebServiceRoute<HB> = Router<(), HB>;

// Due to the issues #1110 and discussion #1818 in Axum v0.6.x,
// the generic type parameter of final router depends all the middleware
// layers added to the router, because they wrap the original http request
// and response body layer by layer, the type parameter `HB` has to match
// that at compile time

pub fn app_web_service<HB>(
    cfg: &WebApiListenCfg,
    rtable: ApiRouteTableType<HB>,
    shr_state: AppSharedState,
) -> (WebServiceRoute<HB>, u16)
where
    HB: HttpBody + Send + 'static,
{
    // the type parameters for shared state and http body should be explicitly annotated,
    // this function creates a router first then specify type of the shared state later
    // at the end of the same function.
    let mut router: Router<AppSharedState, HB> = Router::new();
    let iterator = cfg.routes.iter();
    let filt_fn = |&item: &&WebApiRouteCfg| -> bool {
        let hdlr_label = item.handler.as_str();
        rtable.contains_key(hdlr_label)
    };
    let filtered = iterator.filter(filt_fn);
    let mut num_applied: u16 = 0;
    for item in filtered {
        let hdlr_label = item.handler.as_str();
        if let Some(route) = rtable.get(hdlr_label) {
            let route_cpy: ApiRouteType<HB> = route.clone();
            router = router.route(item.path.as_str(), route_cpy);
            num_applied += 1u16;
        } // 2 different paths might linked to the same handler
    }
    let router = if num_applied > 0 {
        let api_ver_path = String::from("/") + &cfg.api_version;
        Router::new().nest(api_ver_path.as_str(), router)
    } else {
        router
    };
    // DO NOT specify state type at here, Axum converts a router to a leaf service
    // ONLY when the type parameter `S` in `Router` becomes empty tuple `()`.
    // It is counter-intuitive that the `S` means :
    //
    //     "state type that is missing in the router".
    //
    ////let router = router.with_state::<AppSharedState>(shr_state); // will cause error
    let router = router.with_state(shr_state);
    // let service = IntoMakeService{svc:router}; // prohibit
    (router, num_applied)
} // end of fn app_web_service

pub mod middleware {
    use std::fs::File;
    use std::pin::Pin;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use std::time::Duration;

    use axum::http;
    use serde::Deserialize;
    use tower::limit::RateLimitLayer;
    use tower::{Layer, Service};
    use tower_http::cors::CorsLayer;
    use tower_http::limit::RequestBodyLimitLayer;

    use super::{AppError, AppErrorCode, DefaultResult};

    #[derive(Deserialize)]
    struct CorsAllowedOrigin {
        order: String,
    }

    #[allow(non_snake_case)]
    #[derive(Deserialize)]
    struct CorsConfig {
        ALLOWED_ORIGIN: CorsAllowedOrigin,
        ALLOWED_METHODS: Vec<String>,
        ALLOWED_HEADERS: Vec<String>,
        ALLOW_CREDENTIALS: bool,
        PREFLIGHT_MAX_AGE: u64,
    }

    pub struct ShutdownDetection<S, RespBody> {
        inner: S, // inner middleware service wrapped by this service
        flag: Arc<AtomicBool>,
        num_reqs: Arc<AtomicU32>,
        _ghost: std::marker::PhantomData<RespBody>,
    }
    pub struct ShutdownDetectionLayer<RespBody> {
        flag: Arc<AtomicBool>,
        num_reqs: Arc<AtomicU32>,
        _ghost: std::marker::PhantomData<RespBody>,
    }

    pub fn rate_limit(max_conn: u32) -> RateLimitLayer {
        let num = max_conn as u64;
        let period = Duration::from_secs(1);
        RateLimitLayer::new(num, period)
    }

    pub fn cors(cfg_path: String) -> DefaultResult<CorsLayer, AppError> {
        match File::open(cfg_path) {
            Ok(f) => match serde_json::from_reader::<File, CorsConfig>(f) {
                Ok(val) => {
                    let methods = val
                        .ALLOWED_METHODS
                        .iter()
                        .filter_map(|m| match http::Method::from_bytes(m.as_bytes()) {
                            Ok(ms) => Some(ms),
                            Err(_e) => None,
                        })
                        .collect::<Vec<http::Method>>();
                    if val.ALLOWED_METHODS.len() > methods.len() {
                        return Err(AppError {
                            detail: Some("invalid-allowed-method".to_string()),
                            code: AppErrorCode::InvalidInput,
                        });
                    }
                    let headers = val
                        .ALLOWED_HEADERS
                        .iter()
                        .filter_map(|h| match http::HeaderName::from_str(h.as_str()) {
                            Ok(hs) => Some(hs),
                            Err(_e) => None,
                        })
                        .collect::<Vec<http::HeaderName>>();
                    if !headers.contains(&http::header::AUTHORIZATION)
                        || !headers.contains(&http::header::CONTENT_TYPE)
                        || !headers.contains(&http::header::ACCEPT)
                    {
                        return Err(AppError {
                            detail: Some("invalid-allowed-header".to_string()),
                            code: AppErrorCode::InvalidInput,
                        });
                    }
                    let origin = val
                        .ALLOWED_ORIGIN
                        .order
                        .parse::<http::HeaderValue>()
                        .unwrap();
                    let co = CorsLayer::new()
                        .allow_origin(origin)
                        .allow_methods(methods)
                        .allow_headers(headers)
                        .allow_credentials(val.ALLOW_CREDENTIALS)
                        .max_age(Duration::from_secs(val.PREFLIGHT_MAX_AGE));
                    Ok(co)
                }
                Err(e) => Err(AppError {
                    detail: Some(e.to_string()),
                    code: AppErrorCode::InvalidJsonFormat,
                }),
            },
            Err(e) => Err(AppError {
                detail: Some(e.to_string()),
                code: AppErrorCode::IOerror(e.kind()),
            }),
        } // end of file open
    } // end of fn cors_middleware

    pub fn req_body_limit(limit: usize) -> RequestBodyLimitLayer {
        RequestBodyLimitLayer::new(limit)
    }

    pub enum ShutdownExpRespBody<B> {
        Normal {
            inner: B,
        },
        ShuttingDown {
            inner: http_body::Full<axum::body::Bytes>,
        },
    }
    impl<B> ShutdownExpRespBody<B> {
        fn normal(inner: B) -> Self {
            Self::Normal { inner }
        }

        fn error() -> Self {
            let msg = b"server-shutting-down".to_vec();
            let inner = http_body::Full::from(msg);
            Self::ShuttingDown { inner }
        }
    }
    impl<B> http_body::Body for ShutdownExpRespBody<B>
    where
        B: http_body::Body<Data = axum::body::Bytes> + std::marker::Unpin,
    {
        type Data = axum::body::Bytes;
        type Error = B::Error;
        fn poll_data(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<DefaultResult<Self::Data, Self::Error>>> {
            unsafe {
                match self.get_unchecked_mut() {
                    Self::ShuttingDown { inner } => {
                        let pinned = Pin::new(inner);
                        pinned.poll_data(cx).map_err(|err| match err {})
                    }
                    Self::Normal { inner } => {
                        let pinned = Pin::new(inner);
                        pinned.poll_data(cx)
                    }
                }
            } // TODO, improve the code, `Pin::get_unchecked_mut()` is the only function
              // which requires to run in unsafe block
        }

        fn poll_trailers(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<DefaultResult<Option<http::HeaderMap>, Self::Error>> {
            unsafe {
                match self.get_unchecked_mut() {
                    Self::ShuttingDown { inner } => Pin::new(inner)
                        .poll_trailers(cx)
                        .map_err(|err| match err {}),
                    Self::Normal { inner } => Pin::new(inner).poll_trailers(cx),
                }
            }
        }

        fn is_end_stream(&self) -> bool {
            match self {
                Self::ShuttingDown { inner } => inner.is_end_stream(),
                Self::Normal { inner } => inner.is_end_stream(),
            }
        }

        fn size_hint(&self) -> http_body::SizeHint {
            match self {
                Self::ShuttingDown { inner } => inner.size_hint(),
                Self::Normal { inner } => inner.size_hint(),
            }
        }
    } // end of impl http-body Body for ShutdownExpRespBody

    impl<S, RespBody> ShutdownDetection<S, RespBody> {
        fn new(flag: Arc<AtomicBool>, num_reqs: Arc<AtomicU32>, inner: S) -> Self {
            #[allow(clippy::default_constructed_unit_structs)]
            let _ghost = std::marker::PhantomData::default();
            Self {
                inner,
                flag,
                num_reqs,
                _ghost,
            }
        }
    }
    impl<S, REQ, RespBody> Service<REQ> for ShutdownDetection<S, RespBody>
    where
        S: Service<REQ, Response = http::Response<RespBody>>,
        RespBody: http_body::Body,
        <S as Service<REQ>>::Future: std::future::Future + Send + 'static,
        // It is tricky to correctly set constraint on error type from inner service :
        // - it may be converted to box pointer of some trait object, but it would be
        //   good not to change the error struct.
        // - it may also be `Infallible`, which means inner service should never reach
        //   the error condition , in such case I cannot convert  coustom error to
        //   `Infallible` becuase there is no public API in Rust which allows you to do so.
        //
        // [reference]
        // https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md#the-error-type
        // <S as Service<REQ>>::Error: std::error::Error + Send + Sync + 'static ,
        // <S as Service<REQ>>::Error: From<AppError> + Send + Sync + 'static ,
    {
        type Response = http::Response<ShutdownExpRespBody<RespBody>>;
        type Error = S::Error; // tower::BoxError;
        type Future = Pin<
            Box<
                dyn std::future::Future<Output = DefaultResult<Self::Response, Self::Error>> + Send,
            >,
        >;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<DefaultResult<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        fn call(&mut self, req: REQ) -> Self::Future {
            let is_shutting_down = self.flag.load(Ordering::Relaxed);
            if is_shutting_down {
                Box::pin(async {
                    let body = ShutdownExpRespBody::error();
                    let resp = hyper::Response::builder()
                        .status(http::StatusCode::SERVICE_UNAVAILABLE)
                        .body(body)
                        .unwrap();
                    Ok(resp)
                })
            } else {
                let num_reqs_cnt = self.num_reqs.clone();
                let _prev = num_reqs_cnt.fetch_add(1u32, Ordering::Relaxed);
                let inner_fut = self.inner.call(req);
                Box::pin(async move {
                    let orig_resp = inner_fut.await?;
                    let (parts, rbody) = orig_resp.into_parts();
                    let cvt_rbody = ShutdownExpRespBody::normal(rbody);
                    let cvt_resp = http::Response::from_parts(parts, cvt_rbody);
                    let _prev = num_reqs_cnt.fetch_sub(1u32, Ordering::Relaxed);
                    Ok(cvt_resp)
                })
            }
        }
    } // end of impl ShutdownDetection
    impl<RespBody> ShutdownDetectionLayer<RespBody> {
        pub fn new(flag: Arc<AtomicBool>, num_reqs: Arc<AtomicU32>) -> Self {
            #[allow(clippy::default_constructed_unit_structs)]
            let _ghost = std::marker::PhantomData::default();
            Self {
                flag,
                num_reqs,
                _ghost,
            }
        }
        pub fn number_requests(&self) -> Arc<AtomicU32> {
            self.num_reqs.clone()
        }
    }
    impl<S, RespBody> Layer<S> for ShutdownDetectionLayer<RespBody> {
        type Service = ShutdownDetection<S, RespBody>;

        fn layer(&self, inner: S) -> Self::Service {
            Self::Service::new(self.flag.clone(), self.num_reqs.clone(), inner)
        }
    }

    impl<RespBody> Clone for ShutdownDetectionLayer<RespBody> {
        fn clone(&self) -> Self {
            Self {
                flag: self.flag.clone(),
                num_reqs: self.num_reqs.clone(),
                _ghost: self._ghost,
            }
        }
    }
    impl<S, RespBody> Clone for ShutdownDetection<S, RespBody>
    where
        S: Clone,
    {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
                flag: self.flag.clone(),
                num_reqs: self.num_reqs.clone(),
                _ghost: self._ghost,
            }
        }
    }
} // end of inner-module middleware

pub fn net_server_listener(
    mut domain_host: String,
    port: u16,
) -> DefaultResult<HyperSrvBuilder<AddrIncoming>, AppError> {
    if !domain_host.contains(':') {
        domain_host += &":0";
    }
    match domain_host.to_socket_addrs() {
        Ok(mut iterator) => loop {
            match iterator.next() {
                Some(mut addr) => {
                    addr.set_port(port);
                    if let Ok(b) = HyperServer::try_bind(&addr) {
                        break Ok(b);
                    }
                }
                None => {
                    break Err(AppError {
                        detail: Some("failed to bound with all IPs".to_string()),
                        code: AppErrorCode::IOerror(ErrorKind::AddrInUse),
                    })
                }
            }
        }, // end of loop
        Err(e) => Err(AppError {
            detail: Some(e.to_string() + ", domain_host:" + &domain_host),
            code: AppErrorCode::IOerror(ErrorKind::AddrNotAvailable),
        }), // IP not found after domain name resolution
    }
} // end of fn net_server_listener
