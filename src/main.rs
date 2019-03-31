extern crate dotenv;
extern crate env_logger;

extern crate hyper;
extern crate hyper_tls;

extern crate rustc_serialize;
extern crate tungstenite;
extern crate url;

use std::env;
use std::io::{self, Write};
use std::process;

use dotenv::dotenv;
use rustc_serialize::json;
use tungstenite::{connect, Message};
use url::Url;

use hyper::rt::{self, Future, Stream};
use hyper::Client;
use hyper::{Body, Method, Request};
use hyper_tls::HttpsConnector;

fn main() {
  env_logger::init();
  dotenv().ok();

  let relay_key = &env::var("RELAY_KEY").unwrap();
  let relay_secret = &env::var("RELAY_SECRET").unwrap();
  let relay_bucket = &env::var("RELAY_BUCKET").unwrap();

  if relay_key == "" || relay_secret == "" {
    println!("Environment variables RELAY_KEY and RELAY_SECRET must be set");
    process::exit(1);
  }

  let (mut socket, response) = connect(Url::parse("wss://my.webhookrelay.com:443/v1/socket").unwrap()).expect("Can't connect");

  println!("Connected to the server");
  for &(ref header, _ /*value*/) in response.headers.iter() {
    println!("* {}", header);
  }

  let auth_action = AuthStruct {
    action: "auth".to_string(),
    key: relay_key.to_string(),
    secret: relay_secret.to_string(),
  };

  let encoded = json::encode(&auth_action).unwrap();

  // socket.write_message(Message::Text("{'action': 'auth', 'key': 'foo', 'secret': 'bar'}".into())).unwrap();
  socket.write_message(Message::Text(encoded.into())).unwrap();
  loop {
    let msg = socket.read_message().expect("Error reading message");

    println!("Received: {}", msg);

    let event: EventStruct = json::decode(&msg.to_text().unwrap()).unwrap();

    if event.r#type == "status" {
      let event: StatusStruct = json::decode(&msg.to_text().unwrap()).unwrap();
      if event.status == "authenticated" {
        let subscribe_action = SubscribeStruct {
          action: "subscribe".to_string(),
          buckets: [relay_bucket.to_string()],
        };
        let encoded = json::encode(&subscribe_action).unwrap();
        socket.write_message(Message::Text(encoded.into())).unwrap();
      } else if event.status == "ping" {
        let pong_action = PongStruct {
          action: "pong".to_string(),
        };
        let encoded = json::encode(&pong_action).unwrap();
        socket.write_message(Message::Text(encoded.into())).unwrap();
        println!("Ping -> Pong");
      }

    // TODO: if unauthorized - close socket and exit
    } else if event.r#type == "webhook" {
      let webhook: WebhookStruct = json::decode(&msg.to_text().unwrap()).unwrap();
      rt::run(relay(webhook));
    }
  }
  // socket.close(None);
}

fn relay(webhook: WebhookStruct) -> impl Future<Item = (), Error = ()> {
  // let client = Client::new();
  let https = HttpsConnector::new(4).expect("TLS initialization failed");
  let client = Client::builder().build::<_, hyper::Body>(https);

  let uri: hyper::Uri = webhook.meta.output_destination.parse().unwrap();
  println!("url parsed {}", webhook.meta.output_destination.to_string());
  // let mut req = Request::new(webhook.body.to_string());
  let mut req = Request::new(Body::from(webhook.body.to_string()));
  *req.method_mut() = Method::from_bytes(webhook.method.as_bytes()).unwrap();
  *req.uri_mut() = uri.clone();

  client
    .request(req)
    .and_then(|res| {
      println!("Response: {}", res.status());
      println!("Headers: {:#?}", res.headers());

      // The body is a stream, and for_each returns a new Future
      // when the stream is finished, and calls the closure on
      // each chunk of the body...
      res
        .into_body()
        .for_each(|chunk| io::stdout().write_all(&chunk).map_err(|e| panic!("error={}", e)))
    })
    // If all good, just tell the user...
    .map(|_| {
      println!("\n\nDone.");
    })
    // If there was an error, let the user know...
    .map_err(|err| {
      eprintln!("Error {}", err);
    })
}

// Automatically generate `RustcDecodable` and `RustcEncodable` trait
// implementations
#[derive(RustcDecodable, RustcEncodable)]
pub struct AuthStruct {
  action: String,
  key: String,
  secret: String,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct PongStruct {
  action: String,
}

const BUCKETS_SIZE: usize = 1;

#[derive(RustcDecodable, RustcEncodable)]
pub struct SubscribeStruct {
  action: String,
  buckets: [String; BUCKETS_SIZE],
}

// EventStruct - top level struct
#[derive(RustcDecodable, RustcEncodable)]
pub struct EventStruct {
  r#type: String,
}

// StatusStruct is used for generic events such as authenticated/unauthorized
// example struct:
// {
//     "type": "status",
//     "status": "subscribed",
//     "message": "subscribed to buckets: my-1-bucket-name, my-2-bucket-id"
// }
#[derive(RustcDecodable, RustcEncodable)]
pub struct StatusStruct {
  r#type: String,
  status: String,
  message: String,
}

// WebhookStruct is used to send whole webhook data over the socket so it can
// be relayed, example:
// {
//   "type": "webhook",             // event type
//   "meta": {                      // bucket, input and output information
//     "bucked_id": "1593fe5f-45f9-45cc-ba23-675fdc7c1638",
//     "bucket_name": "my-1-bucket-name",
//     "input_id": "b90f2fe9-621d-4290-9e74-edd5b61325dd",
//     "input_name": "Default public endpoint",
//     "output_name": "111",
// 		"output_destination": "http://localhost:8080"
//   },
//   "headers": {                   // request headers
//     "Content-Type": [
//       "application/json"
//     ]
//   },
//   "query": "foo=bar",            // query (ie: /some-path?foo=bar)
//   "body": "{\"hi\": \"there\"}", // request body
//   "method": "PUT"                // request method
// }
#[derive(RustcDecodable, RustcEncodable)]
pub struct WebhookStruct {
  meta: WebhookMetadata,
  r#type: String,
  body: String,
  method: String,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct WebhookMetadata {
  output_name: String,
  output_destination: String,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct WebhookHeaders {
  output_name: String,
  output_destination: String,
}
