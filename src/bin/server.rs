//use log::error;

pub mod greeter {
    use prost_derive::Message;
    include!(concat!(env!("OUT_DIR"), "/greeter.rs"));
}

use greeter::{server, HelloReply, HelloRequest};

use futures::{future, Future, Stream};
use tokio::executor::DefaultExecutor;
use tokio::net::TcpListener;
use tower_grpc::{Request, Response};
use tower_h2::Server;

use rustls::AllowAnyAuthenticatedClient;
use rustls::{RootCertStore, ServerConfig};
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;
use tower_grpc_tls_client_auth_example::{load_certs, load_private_key};


#[derive(Clone, Debug)]
struct Greet;

impl server::Greeter for Greet {
    type SayHelloFuture = future::FutureResult<Response<HelloReply>, tower_grpc::Error>;

    fn say_hello(&mut self, request: Request<HelloRequest>) -> Self::SayHelloFuture {
        println!("REQUEST = {:?}", request);

        let response = Response::new(HelloReply {
            message: "Zomg, it works!".to_string(),
        });

        future::ok(response)
    }
}

pub fn main() {
    let _ = ::env_logger::init();

    let addr = "127.0.0.1:50051".parse().unwrap();
    let bind = TcpListener::bind(&addr).expect("bind");

    let mut client_auth_roots = RootCertStore::empty();
    let roots = load_certs("test-ca/rsa/end.fullchain");
    for root in &roots {
        client_auth_roots.add(&root).unwrap();
    }
    let client_auth = AllowAnyAuthenticatedClient::new(client_auth_roots);

    let mut tls_config = ServerConfig::new(client_auth);
    tls_config.set_single_cert(roots, load_private_key("test-ca/rsa/end.key")).unwrap();
    tls_config.alpn_protocols.push(b"h2".to_vec());
    let tls_config = Arc::new(tls_config);
    let tls_acceptor = TlsAcceptor::from(tls_config);

    let serve = bind
        .incoming()
        .for_each( move |sock| {
            if let Err(e) = sock.set_nodelay(true) {
                return Err(e);
            }

            let connection = tls_acceptor
                .accept(sock)
                .map_err(|e| eprintln!("Error: {:?}", e))
                .and_then(|stream| {
                    let new_service = server::GreeterServer::new(Greet);
                    let h2_settings = Default::default();
                    let mut h2 = Server::new(new_service, h2_settings, DefaultExecutor::current());
                    tokio::spawn(
                        h2.serve(stream)
                            .map_err(|e| eprintln!("Error: {:?}", e)),
                    );
                    Ok(())
                });

            tokio::spawn(connection);

            Ok(())
        })
        .map_err(|e| eprintln!("accept error: {}", e))
        .map(|_| {});

    tokio::run(serve)
}
