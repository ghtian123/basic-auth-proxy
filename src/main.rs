use axum::{
    extract::State,
    http::{Request, Response},
    Router,
};

use axum_server::tls_rustls::RustlsConfig;
use http::Version;
use std::{net::SocketAddr, path::PathBuf};

use hyper::{client::HttpConnector, Body, Client, Uri};
use hyper_rustls::HttpsConnector;

use tower_http::auth::RequireAuthorizationLayer;

use anyhow::{anyhow, Result};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Proxy {
    #[clap(
        short = 'l',
        long = "listen_addr",
        default_value = "0.0.0.0:3000",
        help = "proxy server  listen"
    )]
    listen_addr: SocketAddr,

    #[clap(
        short = 'p',
        long = "proxy_addr",
        default_value = "https://www.baidu.com",
        help = "which addr to proxy"
    )]
    proxy_addr: Uri,

    #[clap(
        short = 'c',
        long = "cert_path",
        default_value = "./",
        help = "cert_path"
    )]
    cert_path: PathBuf,

    #[clap(
        short = 'u',
        long = "user_passwd",
        default_value = "test:test",
        help = "user_passwd to auth,eg: test:test",
        value_parser = parse_user_passwd,
    )]
    user_passwd: (String, String),
}

fn parse_user_passwd(s: &str) -> Result<(String, String)> {
    s.split_once(':')
        .and_then(|(x1, x2)| Some((x1.to_string(), x2.to_string())))
        .ok_or(anyhow!("user_passwd input err: {}", s))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let proxy = Proxy::parse();
    tracing::info!("{:?}", proxy);

    // 证书路径
    let config = RustlsConfig::from_pem_file(
        proxy.cert_path.join("self_signed_certs").join("cert.pem"),
        proxy.cert_path.join("self_signed_certs").join("key.pem"),
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
        .with_state((client, proxy.proxy_addr))
        .layer(RequireAuthorizationLayer::basic(
            proxy.user_passwd.0.as_str(),
            proxy.user_passwd.1.as_str(),
        ));

    tracing::info!("listening on {}", proxy.listen_addr);
    axum_server::bind_rustls(proxy.listen_addr, config)
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
