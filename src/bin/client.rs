use futures::Future;
use tokio::executor::DefaultExecutor;
use tokio::net::tcp::TcpStream;
use tower_grpc::Request;
use tower_h2::client;
use tower_util::MakeService;

use tower_grpc_tls_client_auth_example::{load_certs, load_private_key};

use rustls::ClientSession;
use std::fs;
use std::io::BufReader;
use std::sync::Arc;
use tokio_rustls::TlsStream;
use tokio_rustls::{rustls::ClientConfig, TlsConnector};

pub mod greeter {
    use prost_derive::Message;
    include!(concat!(env!("OUT_DIR"), "/greeter.rs"));
}

pub fn main() {
    let _ = ::env_logger::init();

    let uri: http::Uri = format!("https://localhost:50051").parse().unwrap();

    let h2_settings = Default::default();
    let mut make_client = client::Connect::new(Dst {}, h2_settings, DefaultExecutor::current());

    let say_hello = make_client
        .make_service(())
        .map(move |conn| {
            use greeter::client::Greeter;
            use tower_http::add_origin;

            let conn = add_origin::Builder::new().uri(uri).build(conn).unwrap();

            Greeter::new(conn)
        })
        .and_then(|mut client| {
            use greeter::HelloRequest;

            client
                .say_hello(Request::new(HelloRequest {
                    name: "What is in a name?".to_string(),
                }))
                .map_err(|e| panic!("gRPC request failed; err={:?}", e))
        })
        .and_then(|response| {
            println!("RESPONSE = {:?}", response);
            Ok(())
        })
        .map_err(|e| {
            println!("ERR = {:?}", e);
        });

    tokio::run(say_hello);
}

struct Dst;

impl tokio_connect::Connect for Dst {
    type Connected = TlsStream<TcpStream, ClientSession>;
    type Error = ::std::io::Error;
    type Future = Box<Future<Item = Self::Connected, Error = ::std::io::Error> + Send>;

    fn connect(&self) -> Self::Future {
        let mut pem = BufReader::new(fs::File::open("test-ca/rsa/ca.cert").unwrap());
        let mut config = ClientConfig::new();
        config.root_store.add_pem_file(&mut pem).unwrap();
        config.set_single_client_cert(
            load_certs("test-ca/rsa/client.cert"),
            load_private_key("test-ca/rsa/client.key"),
        );
        config.alpn_protocols.push(b"h2".to_vec());
        let config = Arc::new(config);
        let tls_connector = TlsConnector::from(config);

        let domain = webpki::DNSNameRef::try_from_ascii_str("localhost").unwrap();

        let stream = TcpStream::connect(&([127, 0, 0, 1], 50051).into()).and_then(move |sock| {
            sock.set_nodelay(true).unwrap();
            tls_connector.connect(domain, sock)
        });

        Box::new(stream)
    }
}
