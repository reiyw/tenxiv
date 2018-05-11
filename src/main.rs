#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;
extern crate url;
extern crate reqwest;
extern crate hyper;
extern crate select;
extern crate scraper;
//extern crate regex;

use hyper::header::{Authorization, Bearer};
//use regex::Regex;
use rocket_contrib::Json;
use scraper::{Html, Selector};
use select::document::Document;
use select::predicate::{Attr, Class, Name, Predicate};
use std::{thread, time};
use std::collections::HashMap;
use std::env;
use url::{ParseError, Url};


#[derive(Deserialize)]
struct Message {
    #[serde(rename = "type")]
    typ: String,
    token: String,
    challenge: Option<String>,
    // team_id: Option<String>,
    // api_app_id: Option<String>,
    event: Option<Event>,
    // authed_users: Option<Vec<String>>,
    // event_id: Option<String>,
    // event_time: Option<u32>,
}

#[derive(Deserialize)]
struct Event {
    // #[serde(rename = "type")]
    // typ: String,
    channel: String,
    // user: String,
    message_ts: String,
    links: Vec<Link>,
}

#[derive(Deserialize)]
struct Link {
    domain: String,
    url: String,
}

#[derive(Debug, Serialize)]
struct UnfurlBody {
    channel: String,
    ts: String,
    unfurls: HashMap<String, Attachment>,
}

#[derive(Debug, Serialize)]
struct Attachment {
    text: String,
}

#[derive(Debug)]
struct Article {
    pub id: String,
    pub title: String,
    pub url: String,
    pub authors: Vec<String>,
    pub abst: Option<String>,
}

impl Article {
    fn from_arxiv(url: &str) -> Option<Article> {
        let body = reqwest::get(url).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        // like: "[ID] TITLE"
        let id_and_title = document.select(&Selector::parse("title").unwrap()).next().unwrap().text().collect::<String>();
        let id_and_title = id_and_title.split("] ").collect::<Vec<&str>>();
        let id = id_and_title[0].trim_left_matches('[').to_string();
        let title = id_and_title[1].to_string();

        let authors_s = document.select(&Selector::parse(".authors").unwrap()).next().unwrap().text().collect::<String>().replace("\n", " ").replace("Authors: ", "");
        let authors: Vec<String> = authors_s.split(", ").map(|author| author.trim().to_string()).collect();

        let abst = document.select(&Selector::parse(".abstract").unwrap()).next().unwrap().text().collect::<String>().replace("\n", " ").replace("Abstract: ", "");
        let article = Article { id: id, title: title, url: url.to_string(), authors: authors, abst: Some(abst.trim().to_string()) };
        println!("{:?}", &article);
        Some(article)
    }
}

#[test]
fn test_arxiv() {
    let article = Article::from_arxiv("https://arxiv.org/abs/1803.06643v1").unwrap();
    assert!(article.id == "1803.06643v1".to_string());
    assert!(article.title == "The Web as a Knowledge-base for Answering Complex Questions".to_string());
    assert!(article.url == "https://arxiv.org/abs/1803.06643v1".to_string());
    assert!(article.authors == vec!["Alon Talmor".to_string(), "Jonathan Berant".to_string()]);
    assert!(article.abst == Some("Answering complex questions is a time-consuming activity for humans that requires reasoning and integration of information. Recent work on reading comprehension made headway in answering simple questions, but tackling complex questions is still an ongoing research challenge. Conversely, semantic parsers have been successful at handling compositionality, but only when the information resides in a target knowledge-base. In this paper, we present a novel framework for answering broad and complex questions, assuming answering simple questions is possible using a search engine and a reading comprehension model. We propose to decompose complex questions into a sequence of simple questions, and compute the final answer from the sequence of answers. To illustrate the viability of our approach, we create a new dataset of complex questions, ComplexWebQuestions, and present a model that decomposes questions and interacts with the web to compute an answer. We empirically demonstrate that question decomposition improves performance from 20.8 precision@1 to 27.5 precision@1 on this new dataset.".to_string()));
}

#[post("/", format = "application/json", data = "<message>")]
fn index(message: Json<Message>) -> String {
    match message.0.challenge {
        Some(val) => return val,
        None => ()
    }

    let event: Event = message.0.event.unwrap();

    thread::spawn(move || {
        for link in &event.links {
            let article = match &link.domain[..] {
                "arxiv.org" => Article::from_arxiv(&link.url),
                _ => None
            };
            match article {
                Some(article) => send_unfurl_request(&event.channel, &event.message_ts, article),
                None => ()
            }
        }
    });

    String::new()
}

fn send_unfurl_request(channel: &str, ts: &str, article: Article) {
    // こんな感じで送りたい:
    // POST /api/chat.unfurl
    // Content-type: application/json
    // Authorization: Bearer xoxa-xxxxxxxxx-xxxx
    // {"name":"something-urgent"}
    let attachment = Attachment { text: "hoge".to_string() };
    let unfurls: HashMap<String, Attachment> = vec![
        (article.url, attachment),
    ].into_iter().collect();
    let ub = UnfurlBody { channel: channel.to_string(), ts: ts.to_string(), unfurls: unfurls };

    let client = reqwest::Client::new();
    let mut res = client.post("https://slack.com/api/chat.unfurl")
        .header(Authorization(Bearer {
            token: env::var("SLACK_ACCESS_TOKEN").unwrap()
        }))
        .json(&ub)
        .send().ok().unwrap();
    let content = res.text().unwrap();
    println!("{}", content);
}

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}
