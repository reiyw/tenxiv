#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;


use rocket_contrib::Json;


#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[derive(Deserialize)]
struct Certificate {
    token: String,
    challenge: String,
    #[serde(rename = "type")]
    typ: String,
}

#[post("/", format = "application/json", data = "<certificate>")]
fn verify(certificate: Json<Certificate>) -> String {
    certificate.0.challenge
}

fn main() {
    rocket::ignite().mount("/", routes![index, verify]).launch();
}
