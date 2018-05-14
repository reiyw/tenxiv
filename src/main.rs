#![feature(plugin)]
#![plugin(rocket_codegen)]

//extern crate regex;
extern crate chrono;
extern crate hyper;
extern crate reqwest;
extern crate rocket;
extern crate rocket_contrib;
extern crate scraper;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate url;

use chrono::prelude::*;
use hyper::header::{Authorization, Bearer};
//use regex::Regex;
use rocket_contrib::Json;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::env;
use std::thread;
use url::Url;


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
            ("pdf", article.pdf_en_link),
            ("pdf (ja)", article.pdf_ja_link),
            ("html", article.html_en_link),
            ("html (ja)", article.html_ja_link),
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
            Some(abst) => {
                let words: Vec<String> = abst.split(" ").map(|s| s.to_string()).collect();
                let abst = if words.len() > 40 {
                    words[..39].join(" ") + " ..."
                } else {
                    abst
                };
                fields.push(
                    AttachmentField {
                        title: "Abstract".to_string(),
                        value: abst,
                        short: false,
                    }
                )
            }
            _ => (),
        }

        let (color, footer, footer_icon) = match &article.preserver[..] {
            "arXiv" => ("#B22121".to_string(), article.preserver, Some("http://i.imgur.com/8NYocT8.gif".to_string())),
            "OpenReview" => ("#8B211A".to_string(), article.preserver, None),
            "ACL Anthology" => ("#FD0003".to_string(), article.preserver, Some("http://aclweb.org/anthology/images/acl-logo.gif".to_string())),
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
    pub pdf_en_link: Option<String>,
    pub pdf_ja_link: Option<String>,
    pub html_en_link: Option<String>,
    pub html_ja_link: Option<String>,
    pub bib_link: Option<String>,
    pub date: DateTime<Utc>,
}

impl Article {
    fn from_arxiv(url: &str) -> Option<Article> {
        let abs_link = url.replacen("/pdf/", "/abs/", 1);
        let pdf_en_link = abs_link.replacen("/abs/", "/pdf/", 1);
        let pdf_ja_link = format!("https://translate.google.co.jp/translate?sl=en&tl=ja&js=y&prev=_t&hl=ja&ie=UTF-8&u={}&edit-text=&act=url", &pdf_en_link);

        let body = reqwest::get(&abs_link).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        let title = document.select(&Selector::parse(r#"meta[name="citation_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let id = document.select(&Selector::parse(r#"meta[name="citation_arxiv_id"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();

        let html_en_link = format!("https://www.arxiv-vanity.com/papers/{}/", id);
        let html_ja_link = format!("https://translate.google.co.jp/translate?sl=en&tl=ja&js=y&prev=_t&hl=ja&ie=UTF-8&u={}&edit-text=&act=url", &html_en_link);

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
            pdf_en_link: Some(pdf_en_link),
            pdf_ja_link: Some(pdf_ja_link),
            html_en_link: Some(html_en_link),
            html_ja_link: Some(html_ja_link),
            bib_link: None,
            date,
        };
        println!("{:?}", &article);
        Some(article)
    }

    fn from_openreview(url: &str) -> Option<Article> {
        let parsed_url = Url::parse(url).unwrap();
        let hash_query: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();
        let id = hash_query.get("id").unwrap();

        let abs_link = format!("https://openreview.net/forum?id={}", id);
        let pdf_en_link = format!("https://openreview.net/pdf?id={}", id);
        let pdf_ja_link = format!("https://translate.google.co.jp/translate?sl=en&tl=ja&js=y&prev=_t&hl=ja&ie=UTF-8&u={}&edit-text=&act=url", &pdf_en_link);

        let body = reqwest::get(&abs_link).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        let title = document.select(&Selector::parse(r#"meta[name="citation_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let authors: Vec<_> = document.select(&Selector::parse(r#"meta[name="citation_author"]"#).unwrap()).map(|author| author.value().attr("content").unwrap().to_string()).collect();

        let abst: String = document.select(&Selector::parse(".note-content-value").unwrap()).next().unwrap().text().collect();

        let citation_date_str = document.select(&Selector::parse(r#"meta[name="citation_online_date"]"#).unwrap()).next().unwrap().value().attr("content").unwrap();
        let date = match citation_date_str.split("/").map(|s| s.to_string()).collect::<Vec<String>>().as_slice() {
            [y, m, d] => Utc.ymd(y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap()).and_hms(0, 0, 0),
            _ => Utc::now(),
        };

        let article = Article {
            preserver: "OpenReview".to_string(),
            id: id.to_string(),
            title,
            url: abs_link,
            authors,
            abst: Some(abst.trim().to_string()),
            pdf_en_link: Some(pdf_en_link),
            pdf_ja_link: Some(pdf_ja_link),
            html_en_link: None,
            html_ja_link: None,
            bib_link: None,
            date,
        };
        println!("{:?}", &article);
        Some(article)
    }

    fn from_aclweb(url: &str) -> Option<Article> {
        // /hoge/fuga.pdf -> fuga
        let id = url.rsplitn(2, '/').collect::<Vec<&str>>()[0].split('.').collect::<Vec<&str>>()[0];
        let id_upper = id.to_uppercase();
        let id_lower = id.to_lowercase();

        let abs_link = format!("https://aclanthology.info/papers/{}/{}", &id_upper, &id_lower);
        let pdf_en_link = format!("http://aclweb.org/anthology/{}", &id_upper);
        let pdf_ja_link = format!("https://translate.google.co.jp/translate?sl=en&tl=ja&js=y&prev=_t&hl=ja&ie=UTF-8&u={}&edit-text=&act=url", &pdf_en_link);
        let bib_link = format!("{}.bib", &abs_link);

        let body = reqwest::get(&abs_link).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        let title = document.select(&Selector::parse(r#"meta[name="citation_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let authors: Vec<_> = document.select(&Selector::parse(r#"meta[name="citation_author"]"#).unwrap()).map(|author| author.value().attr("content").unwrap().to_string()).collect();

        let year = document.select(&Selector::parse(r#"meta[name="citation_publication_date"]"#).unwrap()).next().unwrap().value().attr("content").unwrap();
        let date = Utc.ymd(year.parse().unwrap(), 1, 1).and_hms(0, 0, 0);

        let article = Article {
            preserver: "ACL Anthology".to_string(),
            id: id_upper,
            title,
            url: abs_link,
            authors,
            abst: None,
            pdf_en_link: Some(pdf_en_link),
            pdf_ja_link: Some(pdf_ja_link),
            html_en_link: None,
            html_ja_link: None,
            bib_link: Some(bib_link),
            date,
        };
        println!("{:?}", &article);
        Some(article)
    }
}

#[test]
fn test_arxiv() {
    let article = Article::from_arxiv("https://arxiv.org/abs/1803.06643v1").unwrap();
    assert_eq!(article.id, "1803.06643".to_string());
    assert_eq!(article.title, "The Web as a Knowledge-base for Answering Complex Questions".to_string());
    assert_eq!(article.url, "https://arxiv.org/abs/1803.06643v1".to_string());
    assert_eq!(article.authors, vec!["Alon Talmor".to_string(), "Jonathan Berant".to_string()]);
    assert_eq!(article.abst, Some("Answering complex questions is a time-consuming activity for humans that requires reasoning and integration of information. Recent work on reading comprehension made headway in answering simple questions, but tackling complex questions is still an ongoing research challenge. Conversely, semantic parsers have been successful at handling compositionality, but only when the information resides in a target knowledge-base. In this paper, we present a novel framework for answering broad and complex questions, assuming answering simple questions is possible using a search engine and a reading comprehension model. We propose to decompose complex questions into a sequence of simple questions, and compute the final answer from the sequence of answers. To illustrate the viability of our approach, we create a new dataset of complex questions, ComplexWebQuestions, and present a model that decomposes questions and interacts with the web to compute an answer. We empirically demonstrate that question decomposition improves performance from 20.8 precision@1 to 27.5 precision@1 on this new dataset.".to_string()));
    assert_eq!(article.pdf_en_link, Some("https://arxiv.org/pdf/1803.06643v1".to_string()));
}

#[test]
fn test_openreview() {
    let article = Article::from_openreview("https://openreview.net/forum?id=Hy7fDog0b").unwrap();
    assert_eq!(article.id, "Hy7fDog0b".to_string());
    assert_eq!(article.title, "AmbientGAN: Generative models from lossy measurements".to_string());
    assert_eq!(article.url, "https://openreview.net/forum?id=Hy7fDog0b".to_string());
    assert_eq!(article.authors, vec!["Ashish Bora".to_string(), "Eric Price".to_string(), "Alexandros G. Dimakis".to_string()]);
    assert_eq!(article.abst, Some("Generative models provide a way to model structure in complex distributions and have been shown to be useful for many tasks of practical interest. However, current techniques for training generative models require access to fully-observed samples. In many settings, it is expensive or even impossible to obtain fully-observed samples, but economical to obtain partial, noisy observations. We consider the task of learning an implicit generative model given only lossy measurements of samples from the distribution of interest. We show that the true underlying distribution can be provably recovered even in the presence of per-sample information loss for a class of measurement models. Based on this, we propose a new method of training Generative Adversarial Networks (GANs) which we call AmbientGAN. On three benchmark datasets, and for various measurement models, we demonstrate substantial qualitative and quantitative improvements. Generative models trained with our method can obtain $2$-$4$x higher inception scores than the baselines.".to_string()));
    assert_eq!(article.pdf_en_link, Some("https://openreview.net/pdf?id=Hy7fDog0b".to_string()));
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
                "openreview.net" => Article::from_openreview(&link.url),
                "aclweb.org" | "aclanthology.coli.uni-saarland.de" | "aclanthology.info" => Article::from_aclweb(&link.url),
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
