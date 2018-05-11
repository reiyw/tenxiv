#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;
extern crate url;
extern crate reqwest;
extern crate hyper;
extern crate scraper;
//extern crate regex;
extern crate chrono;

use chrono::prelude::*;
use hyper::header::{Authorization, Bearer};
//use regex::Regex;
use rocket_contrib::Json;
use scraper::{Html, Selector};
use std::thread;
use std::collections::HashMap;
use std::env;


#[derive(Deserialize)]
struct Message {
    // #[serde(rename = "type")]
    // typ: String,
    // token: String,
    challenge: Option<String>,
    // team_id: Option<String>,
    // api_app_id: Option<String>,
    event: Option<Event>,
    // authed_users: Option<Vec<String>>,
    // event_id: Option<String>,
    // event_time: Option<u64>,
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
struct UnfurlRequest {
    channel: String,
    ts: String,
    unfurls: HashMap<String, Attachment>,
}

#[derive(Debug, Serialize)]
struct Attachment {
    text: String,
    color: String,
    author_name: String,
    title: String,
    title_link: String,
    fields: Vec<AttachmentField>,
    footer: String,
    footer_icon: Option<String>,
    ts: i64,
}

impl Attachment {
    fn new(article: Article) -> Attachment {
        let text = [
            ("pdf", article.pdf_link),
            ("html", article.html_link),
            ("bib", article.bib_link),
        ].iter().flat_map(|(key, link)| {
            match link {
                Some(url) => Some((key, url)),
                None => None,
            }
        }).map(|(key, link)| format!("<{}|{}>", link, key)).collect::<Vec<String>>().join(" | ");

        let author_name = article.authors.join(", ");

        let title = format!("[{}] {}", article.id, article.title);

        let mut fields: Vec<AttachmentField> = Vec::new();
        match article.abst {
            Some(abst) => fields.push(
                AttachmentField {
                    title: "Abstract".to_string(),
                    value: abst,
                    short: false,
                }
            ),
            _ => (),
        }

        let (color, footer, footer_icon) = match &article.preserver[..] {
            "arXiv" => ("#B22121".to_string(), article.preserver, Some("http://i.imgur.com/8NYocT8.gif".to_string())),
            _ => ("#DDDDDD".to_string(), article.preserver, None),
        };

        let attachment = Attachment {
            text,
            color,
            author_name,
            title,
            title_link: article.url,
            fields,
            footer,
            footer_icon,
            ts: article.date.timestamp(),
        };
        println!("{:?}", &attachment);
        attachment
    }
}

#[derive(Debug, Serialize)]
struct AttachmentField {
    title: String,
    value: String,
    short: bool,
}

#[derive(Debug)]
struct Article {
    pub preserver: String,
    pub id: String,
    pub title: String,
    pub url: String,
    pub authors: Vec<String>,
    pub abst: Option<String>,
    pub pdf_link: Option<String>,
    pub html_link: Option<String>,
    pub bib_link: Option<String>,
    pub date: DateTime<Utc>,
}

impl Article {
    fn from_arxiv(url: &str) -> Option<Article> {
        let abs_link = url.replacen("/pdf/", "/abs/", 1);
        let pdf_link = abs_link.replacen("/abs/", "/pdf/", 1);

        let body = reqwest::get(&abs_link).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        let title = document.select(&Selector::parse(r#"meta[name="citation_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let id = document.select(&Selector::parse(r#"meta[name="citation_arxiv_id"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();

        let html_link = format!("https://www.arxiv-vanity.com/papers/{}/", id);

        let authors_s = document.select(&Selector::parse(".authors").unwrap()).next().unwrap().text().collect::<String>().replace("\n", " ").replacen("Authors: ", "", 1);
        let authors: Vec<String> = authors_s.split(", ").map(|author| author.trim().to_string()).collect();

        let abst = document.select(&Selector::parse(".abstract").unwrap()).next().unwrap().text().collect::<String>().replace("\n", " ").replacen("Abstract: ", "", 1);

        let citation_date_str = document.select(&Selector::parse(r#"meta[name="citation_date"]"#).unwrap()).next().unwrap().value().attr("content").unwrap();
        let date = match citation_date_str.split("/").map(|s| s.to_string()).collect::<Vec<String>>().as_slice() {
            [y, m, d] => Utc.ymd(y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap()).and_hms(0, 0, 0),
            _ => Utc::now(),
        };

        let article = Article {
            preserver: "arXiv".to_string(),
            id,
            title,
            url: abs_link,
            authors,
            abst: Some(abst.trim().to_string()),
            pdf_link: Some(pdf_link),
            html_link: Some(html_link),
            bib_link: None,
            date,
        };
        println!("{:?}", &article);
        Some(article)
    }
}

#[test]
fn test_arxiv() {
    let article = Article::from_arxiv("https://arxiv.org/abs/1803.06643v1").unwrap();
    assert_eq!(article.id, "1803.06643v1".to_string());
    assert_eq!(article.title, "The Web as a Knowledge-base for Answering Complex Questions".to_string());
    assert_eq!(article.url, "https://arxiv.org/abs/1803.06643v1".to_string());
    assert_eq!(article.authors, vec!["Alon Talmor".to_string(), "Jonathan Berant".to_string()]);
    assert_eq!(article.abst, Some("Answering complex questions is a time-consuming activity for humans that requires reasoning and integration of information. Recent work on reading comprehension made headway in answering simple questions, but tackling complex questions is still an ongoing research challenge. Conversely, semantic parsers have been successful at handling compositionality, but only when the information resides in a target knowledge-base. In this paper, we present a novel framework for answering broad and complex questions, assuming answering simple questions is possible using a search engine and a reading comprehension model. We propose to decompose complex questions into a sequence of simple questions, and compute the final answer from the sequence of answers. To illustrate the viability of our approach, we create a new dataset of complex questions, ComplexWebQuestions, and present a model that decomposes questions and interacts with the web to compute an answer. We empirically demonstrate that question decomposition improves performance from 20.8 precision@1 to 27.5 precision@1 on this new dataset.".to_string()));
    assert_eq!(article.pdf_link, Some("https://arxiv.org/pdf/1803.06643v1".to_string()));
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
                _ => None,
            };
            let attachment = match article {
                Some(article) => Some(Attachment::new(article)),
                None => None,
            };
            match attachment {
                Some(attachment) => send_unfurl_request(&event.channel, &event.message_ts, &link.url, attachment),
                None => (),
            };
        }
    });

    String::new()
}

fn send_unfurl_request(channel: &str, ts: &str, url: &str, attachment: Attachment) {
    // こんな感じで送りたい:
    // POST /api/chat.unfurl
    // Content-type: application/json
    // Authorization: Bearer xoxa-xxxxxxxxx-xxxx
    // {"name":"something-urgent"}
    let unfurls: HashMap<String, Attachment> = vec![
        (url.to_string(), attachment),
    ].into_iter().collect();
    let ur = UnfurlRequest { channel: channel.to_string(), ts: ts.to_string(), unfurls };

    let client = reqwest::Client::new();
    let mut res = client.post("https://slack.com/api/chat.unfurl")
        .header(Authorization(Bearer {
            token: env::var("SLACK_ACCESS_TOKEN").unwrap()
        }))
        .json(&ur)
        .send().ok().unwrap();
    let content = res.text().unwrap();
    println!("{}", content);
}

fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}
