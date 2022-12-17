use axum::{
    extract::State,
    http::{Request, Response},
    Router,
};

use axum_server::tls_rustls::RustlsConfig;
use http::Version;
use std::{net::SocketAddr, path::PathBuf};

use hyper::{client::HttpConnector, Body};
use tower_http::auth::RequireAuthorizationLayer;
type Client = hyper::client::Client<HttpConnector, Body>;

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
                .num_args(1..)
                .default_value("0.0.0.0:3000"),
        )
        .arg(
            Arg::new("proxy_addr")
                .short('p')
                .long("proxy_addr")
                .help("which addr to proxy")
                .action(ArgAction::Set)
                .num_args(1..)
                .default_value("14.215.177.38"),
        )
        .arg(
            Arg::new("cert_path")
                .short('c')
                .long("cert_path")
                .help("cert path")
                .action(ArgAction::Set)
                .num_args(1..)
                .default_value(env!("CARGO_MANIFEST_DIR")),
        )
        .arg(
            Arg::new("user_passwd")
                .short('u')
                .long("user_passwd")
                .help("user_passwd to auth,eg: test:test")
                .action(ArgAction::Set)
                .num_args(1..)
                .default_value("test:test"),
        )
        .get_matches();

    let proxy = matches
        .get_one::<String>("proxy_addr")
        .expect("default proxy there is always a value");

    tracing::info!("proxy addr -->{}", proxy);

    let listen = matches
        .get_one::<String>("listen_addr")
        .expect("default listen addr there is always a value");

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
    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(
        cert_path.join("self_signed_certs").join("cert.pem"),
        cert_path.join("self_signed_certs").join("key.pem"),
    )
    .await
    .unwrap();

    let client = Client::new();

    let app = Router::new()
        .fallback(reserve)
        .with_state((client, proxy.clone()))
        .layer(RequireAuthorizationLayer::basic(user, passwd));

    let addr = listen.parse::<SocketAddr>().unwrap();
    tracing::info!("listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn reserve(
    State((client, proxy)): State<(Client, String)>,
    mut req: Request<Body>,
) -> Response<Body> {
    tracing::info!("proxy request-->{:?}", req.headers());

    let uri_string = format!(
        "http://{}{}",
        proxy,
        req.uri()
            .path_and_query()
            .map(|x| x.as_str())
            .unwrap_or("/")
    );

    let uri = uri_string.parse().unwrap();
    *req.uri_mut() = uri;

    //代理服务端可能不支持http2
    *req.version_mut() = Version::HTTP_11;

    tracing::info!("proxy to ->{}", &uri_string);

    client.request(req).await.unwrap()
}
