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

use openssl::nid::Nid;
use openssl::x509::X509;
use rustls::Certificate;
use rustls::Session;

#[derive(Clone, Debug)]
struct Greet {
    peer: Option<String>,
}

impl Greet {
    pub fn from_certificates(certs: Option<Vec<Certificate>>) -> Self {
        if let Some(certs) = certs {
            for cert in certs {
                let x509 = X509::from_der(&cert.0).unwrap();
                let subject_name = x509.subject_name();
                let common_name = subject_name
                    .entries_by_nid(Nid::COMMONNAME)
                    .last()
                    .unwrap()
                    .data()
                    .as_utf8()
                    .unwrap()
                    .to_string();
                return Greet {
                    peer: Some(common_name),
                };
            }
        }
        Greet { peer: None }
    }
}

impl server::Greeter for Greet {
    type SayHelloFuture = future::FutureResult<Response<HelloReply>, tower_grpc::Error>;

    fn say_hello(&mut self, request: Request<HelloRequest>) -> Self::SayHelloFuture {
        println!("REQUEST = {:?}", request);

        let msg = if let Some(peer) = &self.peer {
            if *peer == request.get_ref().name {
                format!("Zomg, it works! {}", peer)
            } else {
                format!("YOU ARE NOT {}", peer)
            }
        } else {
            "NO PEER FOUND IN SESSION".to_string()
        };

        let response = Response::new(HelloReply {
            message: msg.to_string(),
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
    tls_config
        .set_single_cert(roots, load_private_key("test-ca/rsa/end.key"))
        .unwrap();
    tls_config.alpn_protocols.push(b"h2".to_vec());
    let tls_config = Arc::new(tls_config);
    let tls_acceptor = TlsAcceptor::from(tls_config);

    let serve = bind
        .incoming()
        .for_each(move |sock| {
            if let Err(e) = sock.set_nodelay(true) {
                return Err(e);
            }

            let connection = tls_acceptor
                .accept(sock)
                .map_err(|e| eprintln!("TLS Accept Error: {:?}", e))
                .and_then(|stream| {
                    let greet = {
                        let (_, session) = stream.get_ref();
                        Greet::from_certificates(session.get_peer_certificates())
                    };

                    let new_service = server::GreeterServer::new(greet);
                    let h2_settings = Default::default();
                    let mut h2 = Server::new(new_service, h2_settings, DefaultExecutor::current());
                    h2.serve(stream).map_err(|e| eprintln!("H2 Serve Error: {:?}", e))
                });

            tokio::spawn(connection);

            Ok(())
        })
        .map_err(|e| eprintln!("accept error: {}", e))
        .map(|_| {});

    tokio::run(serve)
}
