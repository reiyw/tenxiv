#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

use rocket_contrib::Json;
use std::thread;


#[derive(Deserialize)]
struct Message {
    #[serde(rename = "type")]
    typ: String,
    token: String,
    challenge: Option<String>,
    team_id: Option<String>,
    api_app_id: Option<String>,
    event: Option<Event>,
    authed_users: Option<Vec<String>>,
    event_id: Option<String>,
    event_time: Option<u32>,
}

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    typ: String,
    channel: String,
    user: String,
    message_ts: String,
    links: Vec<Link>,
}

#[derive(Deserialize)]
struct Link {
    domain: String,
    url: String,
}

#[post("/", format = "application/json", data = "<message>")]
fn index(message: Json<Message>) -> String {
    match message.0.challenge {
        Some(val) => return val,
        None => ()
    }

    let event: Event = message.0.event.unwrap();
    let links = event.links;
    thread::spawn(move || {
        for link in links {
            retrieve(link.url);
        }
    });

    String::new()
}

fn retrieve(url: String) -> String {
    thread::sleep_ms(2000);
    println!("{}", url);
    url
}

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}
