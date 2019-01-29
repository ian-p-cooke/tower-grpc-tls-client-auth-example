
use futures::Future;
use tokio::executor::DefaultExecutor;
use tokio::net::tcp::{ConnectFuture, TcpStream};
use tower_grpc::Request;
use tower_h2::client;
use tower_util::MakeService;

pub mod greeter {
    use prost_derive::Message;
    include!(concat!(env!("OUT_DIR"), "/greeter.rs"));
}

pub fn main() {
    let _ = ::env_logger::init();

    let uri: http::Uri = format!("http://localhost:50051").parse().unwrap();

    let h2_settings = Default::default();
    let mut make_client = client::Connect::new(Dst, h2_settings, DefaultExecutor::current());

    let say_hello = make_client.make_service(())
        .map(move |conn| {
            use greeter::client::Greeter;
            use tower_http::add_origin;

            let conn = add_origin::Builder::new()
                .uri(uri)
                .build(conn)
                .unwrap();

            Greeter::new(conn)
        })
        .and_then(|mut client| {
            use greeter::HelloRequest;

            client.say_hello(Request::new(HelloRequest {
                name: "What is in a name?".to_string(),
            })).map_err(|e| panic!("gRPC request failed; err={:?}", e))
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
    type Connected = TcpStream;
    type Error = ::std::io::Error;
    type Future = ConnectFuture;

    fn connect(&self) -> Self::Future {
        TcpStream::connect(&([127, 0, 0, 1], 50051).into())
    }
}