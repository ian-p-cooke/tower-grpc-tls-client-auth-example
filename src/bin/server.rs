use log::error;

pub mod greeter {
    use prost_derive::Message;
    include!(concat!(env!("OUT_DIR"), "/greeter.rs"));
}

use greeter::{server, HelloRequest, HelloReply};

use futures::{future, Future, Stream};
use tokio::executor::DefaultExecutor;
use tokio::net::TcpListener;
use tower_h2::Server;
use tower_grpc::{Request, Response};

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

    let new_service = server::GreeterServer::new(Greet);

    let h2_settings = Default::default();
    let mut h2 = Server::new(new_service, h2_settings, DefaultExecutor::current());

    let addr = "127.0.0.1:50051".parse().unwrap();
    let bind = TcpListener::bind(&addr).expect("bind");

    let serve = bind.incoming()
        .for_each(move |sock| {
            if let Err(e) = sock.set_nodelay(true) {
                return Err(e);
            }

            let serve = h2.serve(sock);
            tokio::spawn(serve.map_err(|e| error!("h2 error: {:?}", e)));

            Ok(())
        })
        .map_err(|e| eprintln!("accept error: {}", e));

    tokio::run(serve)
}