use actix_http::Request;
use actix_service::IntoServiceFactory;
use actix_web::body::MessageBody;
use actix_web::dev::{AppConfig, Response, ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::web;
use actix_web::{App, HttpServer};

use crate::api::web::AppRouteTable;

/*
 * the original implementation does not intend to let users transfer `App` object
 * around code functions, it is tricky to do so.
 *
 * The test example in the following FAQ demonstrates how to do this :
 * https://github.com/actix/actix-web/wiki/FAQ#how-can-i-return-app-from-a-function--why-is-appentry-private
 *
 * relavant issues (in actix-web github)
 * #780 #1005 #1156 #2039 #2073 #2082 #2301
 *
 * TODO
 * - support multiple versions of route-tables and configurations
 * */
pub fn app_web_service(
    mut route_table: AppRouteTable,
    cfg: Vec<(String, String)>,
) -> (
    App<
        impl ServiceFactory<
            ServiceRequest,
            Response = ServiceResponse<impl MessageBody>,
            Error = actix_web::error::Error,
            Config = (),
            InitError = (),
        >,
    >,
    usize,
) {
    let mut num_applied = 0usize;
    let num_applied_p = &mut num_applied;
    let cfg_fn = move |c: &mut web::ServiceConfig| {
        *num_applied_p = cfg
            .into_iter()
            .filter_map(|(path, inner_label)| {
                route_table
                    .entries
                    .remove(inner_label.as_str())
                    .map(|found| (path, found))
            })
            .map(|(path, route_found)| {
                c.route(path.as_str(), route_found);
            })
            .count();
    };
    let path_prefix = format!("/{}", route_table.version.as_str());
    let v_scope = web::scope(path_prefix.as_str()).configure(cfg_fn);
    let app = App::new().service(v_scope);
    (app, num_applied)
}

pub fn net_server_listener<F, I, S, B>(
    app_init_cb: F,
    domain_host: &str,
    port: u16,
) -> HttpServer<F, I, S, B>
where
    F: Fn() -> I + Clone + Send + 'static,
    I: IntoServiceFactory<S, Request>,
    S: ServiceFactory<Request, Config = AppConfig> + 'static,
    S::Error: Into<actix_web::error::Error>,
    S::InitError: std::fmt::Debug,
    S::Response: Into<Response<B>>,
    B: MessageBody + 'static,
{
    let domain = format!("{domain_host}:{port}");
    let result = HttpServer::new(app_init_cb).bind(domain);
    result.unwrap()
}
