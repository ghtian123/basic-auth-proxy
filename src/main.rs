use axum::{
    extract::State,
    http::{Request, Response},
    Router,
};

use axum_server::tls_rustls::RustlsConfig;
use http::Version;
use std::{net::SocketAddr, path::PathBuf};

use hyper::Client;
use hyper::{client::HttpConnector, Body, Uri};
use hyper_rustls::HttpsConnector;

use tower_http::auth::RequireAuthorizationLayer;

use clap::{Arg, ArgAction, Command};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let matches = Command::new("proxy server")
        .arg(
            Arg::new("listen_addr")
                .short('l')
                .long("listen_addr")
                .help("which addr to listen")
                .action(ArgAction::Set)
                .num_args(1)
                .default_value("0.0.0.0:3000"),
        )
        .arg(
            Arg::new("proxy_addr")
                .short('p')
                .long("proxy_addr")
                .help("which addr to proxy")
                .action(ArgAction::Set)
                .num_args(1)
                .default_value("https://www.baidu.com"),
        )
        .arg(
            Arg::new("cert_path")
                .short('c')
                .long("cert_path")
                .help("cert path")
                .action(ArgAction::Set)
                .num_args(1)
                .default_value(env!("CARGO_MANIFEST_DIR")),
        )
        .arg(
            Arg::new("user_passwd")
                .short('u')
                .long("user_passwd")
                .help("user_passwd to auth,eg: test:test")
                .action(ArgAction::Set)
                .num_args(1)
                .default_value("test:test"),
        )
        .get_matches();

    let proxy = matches
        .get_one::<String>("proxy_addr")
        .expect("default proxy there is always a value")
        .parse::<Uri>()
        .expect("proxy uri");

    tracing::info!("proxy addr -->{}", proxy);

    let listen = matches
        .get_one::<String>("listen_addr")
        .expect("default listen addr there is always a value")
        .parse::<SocketAddr>()
        .expect("listen addr is not socket addr");

    tracing::info!("listen addr -->{}", listen);
    let cert_path = PathBuf::from(
        matches
            .get_one::<String>("cert_path")
            .expect("default cert path there is always a value"),
    );

    tracing::info!("cert path-->{:?}", cert_path);

    let (user, passwd) = matches
        .get_one::<String>("user_passwd")
        .expect("default user passwd is always a value")
        .split_once(':')
        .unwrap();

    tracing::info!("user->{} passwd->{}", user, passwd);

    // 证书路径
    let config = RustlsConfig::from_pem_file(
        cert_path.join("self_signed_certs").join("cert.pem"),
        cert_path.join("self_signed_certs").join("key.pem"),
    )
    .await
    .unwrap();

    //hyper 构建https 客户端
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();

    let client: Client<HttpsConnector<HttpConnector>, hyper::Body> = Client::builder().build(https);

    let app = Router::new()
        .fallback(reserve)
        .with_state((client, proxy))
        .layer(RequireAuthorizationLayer::basic(user, passwd));

    tracing::info!("listening on {}", listen);
    axum_server::bind_rustls(listen, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn reserve(
    State((client, proxy)): State<(Client<HttpsConnector<HttpConnector>, hyper::Body>, Uri)>,
    mut req: Request<Body>,
) -> Response<Body> {
    tracing::info!("proxy request-->{:?}", req.headers());

    //https 代理端域名和证书必须匹配
    let mut new_uri = Uri::builder().authority(proxy.authority().unwrap().clone());

    new_uri = new_uri.path_and_query(
        req.uri()
            .path_and_query()
            .map(|x| x.as_str())
            .unwrap_or("/"),
    );

    new_uri = new_uri.scheme(proxy.scheme_str().unwrap_or("http"));

    tracing::info!("proxy to ->{:?}", &new_uri);

    *req.uri_mut() = new_uri.build().unwrap();

    //代理服务端可能不支持http2
    *req.version_mut() = Version::HTTP_11;

    let response = match client.request(req).await {
        Ok(response) => response,
        Err(e) => {
            tracing::error!("proxy err->{:?}", e);
            Response::builder()
                .status(503)
                .body("503 Service Unavailable".into())
                .expect("infallible response")
        }
    };
    response
}
