extern crate env_logger;
extern crate rustc_serialize;
extern crate tungstenite;
extern crate url;
extern crate dotenv;

use rustc_serialize::json;
use tungstenite::{connect, Message};
use url::Url;
use dotenv::dotenv;
use std::env;
use std::process;

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
  println!("Response HTTP code: {}", response.code);
  println!("Response contains the following headers:");
  for &(ref header, _ /*value*/) in response.headers.iter() {
    println!("* {}", header);
  }

  let auth_action = AuthStruct {
    action: "auth".to_string(),
    key: relay_key.to_string(),   
    secret: relay_secret.to_string(),
  };

  let encoded = json::encode(&auth_action).unwrap();

  println!("{}", encoded);

  // socket.write_message(Message::Text("{'action': 'auth', 'key': 'foo', 'secret': 'bar'}".into())).unwrap();
  socket.write_message(Message::Text(encoded.into())).unwrap();
  loop {
    let msg = socket.read_message().expect("Error reading message");

    let event: StatusStruct = json::decode(&msg.to_text().unwrap()).unwrap();    
    if event.r#type == "status" {
      println!("Received status message");

      if event.status == "authenticated" {
        let subscribe_action = SubscribeStruct {
          action: "subscribe".to_string(),
          buckets: [relay_bucket.to_string()],
        };

         // subscribing to buckets
        let encoded = json::encode(&subscribe_action).unwrap();
        socket.write_message(Message::Text(encoded.into())).unwrap();
      }

      // TODO: answer to pings with pongs

      // TODO: if unauthorized - close socket and exit
     
      
    } else if event.r#type == "webhook" {
      // deserialize into a webhook msg
      println!("Received webhook message");
    }
    println!("Received: {}", msg);
  }
    // socket.close(None);
}

// Automatically generate `RustcDecodable` and `RustcEncodable` trait
// implementations
#[derive(RustcDecodable, RustcEncodable)]
pub struct AuthStruct {
  action: String,
  key: String,
  secret: String,
}

const BUCKETS_SIZE: usize = 1;

#[derive(RustcDecodable, RustcEncodable)]
pub struct SubscribeStruct {
  action: String,
  buckets: [String; BUCKETS_SIZE],
}

// StatusStruct is used for generic events such as authenticated/unaithorized
#[derive(RustcDecodable, RustcEncodable)]
pub struct StatusStruct {
  r#type: String,
  status: String,
  message: String,
}

#[derive(RustcDecodable, RustcEncodable)]
pub struct WebhookStruct {
  r#type: String,
  body: String,
  method: String,
}
