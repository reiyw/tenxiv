#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

//extern crate regex;
extern crate chrono;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate quick_xml;
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
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader as XmlReader;
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
    token: String,
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
    fn new(article: Article) -> Self {
        let text = [
            ("abstract (ja)", article.url_ja),
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
        match article.volume {
            Some(volume) => fields.push(
                AttachmentField {
                    title: "Volume".to_string(),
                    value: volume,
                    short: false,
                }
            ),
            None => (),
        }

        let (color, footer, footer_icon) = match &article.preserver[..] {
            "arXiv" => ("#B22121".to_string(), article.preserver, Some("http://i.imgur.com/8NYocT8.gif".to_string())),
            "OpenReview" => ("#8B211A".to_string(), article.preserver, None),
            "ACL Anthology" => ("#FD0003".to_string(), article.preserver, Some("http://aclweb.org/anthology/images/acl-logo.gif".to_string())),
            "ACM" => ("#638F36".to_string(), article.preserver, Some("https://www.google.com/s2/favicons?domain=dl.acm.org".to_string())),
            "NIPS Proceedings" => ("#F1652D".to_string(), article.preserver, Some("https://www.google.com/s2/favicons?domain=papers.nips.cc".to_string())),
            "PMLR" => ("#112567".to_string(), article.preserver, Some("http://proceedings.mlr.press/img/favicon.ico".to_string())),
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

#[derive(Debug, Clone)]
struct Article {
    pub preserver: String,
    pub id: String,
    pub title: String,
    pub volume: Option<String>,
    pub url: String,
    pub url_ja: Option<String>,
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
    fn from_arxiv(url: &str) -> Option<Self> {
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
            volume: None,
            url_ja: Some(convert_google_translation_url(&abs_link[..])),
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

    fn from_openreview(url: &str) -> Option<Self> {
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
            volume: None,
            url_ja: Some(convert_google_translation_url(&abs_link[..])),
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

    fn from_aclweb(url: &str) -> Option<Self> {
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
        let volume = document.select(&Selector::parse(r#"meta[name="citation_journal_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let authors: Vec<_> = document.select(&Selector::parse(r#"meta[name="citation_author"]"#).unwrap()).map(|author| author.value().attr("content").unwrap().to_string()).collect();

        let year = document.select(&Selector::parse(r#"meta[name="citation_publication_date"]"#).unwrap()).next().unwrap().value().attr("content").unwrap();
        let date = Utc.ymd(year.parse().unwrap(), 1, 1).and_hms(0, 0, 0);

        let article = Article {
            preserver: "ACL Anthology".to_string(),
            id: id_upper,
            title,
            volume: Some(volume),
            url: abs_link,
            url_ja: None,
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

    fn from_acm(url: &str) -> Option<Self> {
        // TODO: implement
        let parsed_url = Url::parse(url).unwrap();
        let hash_query: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();
        let id = hash_query.get("id").unwrap();

        let abs_link = format!("https://dl.acm.org/citation.cfm?id={}", id);

        let body = reqwest::get(&abs_link).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        let pdf_en_link = document.select(&Selector::parse(r#"meta[name="citation_pdf_url"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let pdf_ja_link = convert_google_translation_url(&pdf_en_link);

        let title = document.select(&Selector::parse(r#"meta[name="citation_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let authors_s = document.select(&Selector::parse(r#"meta[name="citation_authors"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let authors: Vec<String> = authors_s.split("; ").map(|author| author.trim().to_string()).collect();

        let citation_date_str = document.select(&Selector::parse(r#"meta[name="citation_date"]"#).unwrap()).next().unwrap().value().attr("content").unwrap();
        let date = match citation_date_str.split("/").map(|s| s.to_string()).collect::<Vec<String>>().as_slice() {
            [m, d, y] => Utc.ymd(y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap()).and_hms(0, 0, 0),
            _ => Utc::now(),
        };

        let article = Article {
            preserver: "ACM".to_string(),
            id: id.to_string(),
            title,
            volume: None,
            url_ja: Some(convert_google_translation_url(&abs_link)),
            url: abs_link,
            authors,
            abst: None,
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

    fn from_nips(url: &str) -> Option<Self> {
        // /hoge/XXXX-fuga.pdf -> XXXX
        let paths: Vec<&str> = url.rsplitn(3, '/').collect();
        let mut id_title = if paths[0] == "bibtex" {
            paths[1]
        } else {
            paths[0]
        }.to_string();
        if id_title.ends_with(".pdf") {
            let new_len = id_title.len() - 4;
            id_title.truncate(new_len);
        }
        let id = id_title.splitn(2, '-').collect::<Vec<&str>>()[0].to_string();

        let abs_link = format!("http://papers.nips.cc/paper/{}", &id_title);
        let pdf_en_link = format!("{}.pdf", &abs_link);
        let pdf_ja_link = format!("https://translate.google.co.jp/translate?sl=en&tl=ja&js=y&prev=_t&hl=ja&ie=UTF-8&u={}&edit-text=&act=url", &pdf_en_link);
        let bib_link = format!("{}/bibtex", &abs_link);

        let body = reqwest::get(&abs_link).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        let title = document.select(&Selector::parse(r#"meta[name="citation_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let volume = document.select(&Selector::parse(r#"meta[name="citation_conference_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let authors: Vec<_> = document.select(&Selector::parse(r#"meta[name="citation_author"]"#).unwrap()).map(|author| author.value().attr("content").unwrap().to_string()).collect();

        let abst: String = document.select(&Selector::parse(".abstract").unwrap()).next().unwrap().text().collect();

        let year = document.select(&Selector::parse(r#"meta[name="citation_publication_date"]"#).unwrap()).next().unwrap().value().attr("content").unwrap();
        let date = Utc.ymd(year.parse().unwrap(), 1, 1).and_hms(0, 0, 0);

        let article = Article {
            preserver: "NIPS Proceedings".to_string(),
            id,
            title,
            volume: Some(volume),
            url_ja: Some(convert_google_translation_url(&abs_link[..])),
            url: abs_link,
            authors,
            abst: Some(abst),
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

    fn from_pmlr(url: &str) -> Option<Self> {
        // /vXX/fuga.html -> vXX/fuga
        let mut url = url.to_string();
        if url.ends_with(".pdf") {
            let new_len = url.len() - 4;
            url.truncate(new_len);
        } else if url.ends_with(".html") {
            let new_len = url.len() - 5;
            url.truncate(new_len);
        }
        let paths: Vec<&str> = url.rsplitn(3, '/').collect();
        let id = format!("{}/{}", paths[1], paths[0]);

        let abs_link = format!("http://proceedings.mlr.press/{}", &id);
        let pdf_en_link = format!("{}.pdf", &abs_link);
        let pdf_ja_link = convert_google_translation_url(&pdf_en_link);
        let html_en_link = None;
        let html_ja_link = None;
        let bib_link = None;

        let body = reqwest::get(&abs_link).unwrap().text().unwrap();
        let document = Html::parse_document(&body);

        let title = document.select(&Selector::parse(r#"meta[name="citation_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let volume = document.select(&Selector::parse(r#"meta[name="citation_conference_title"]"#).unwrap()).next().unwrap().value().attr("content").unwrap().to_string();
        let authors: Vec<_> = document.select(&Selector::parse(r#"meta[name="citation_author"]"#).unwrap()).map(|author| author.value().attr("content").unwrap().to_string()).collect();

        let abst: String = document.select(&Selector::parse(".abstract").unwrap()).next().unwrap().text().collect::<String>().trim().to_string();

        let citation_date_str = document.select(&Selector::parse(r#"meta[name="citation_publication_date"]"#).unwrap()).next().unwrap().value().attr("content").unwrap();
        let date = match citation_date_str.split("/").map(|s| s.to_string()).collect::<Vec<String>>().as_slice() {
            [y, m, d] => Utc.ymd(y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap()).and_hms(0, 0, 0),
            _ => Utc::now(),
        };

        let article = Article {
            preserver: "PMLR".to_string(),
            id,
            title,
            volume: Some(volume),
            url_ja: Some(convert_google_translation_url(&abs_link[..])),
            url: abs_link,
            authors,
            abst: Some(abst),
            pdf_en_link: Some(pdf_en_link),
            pdf_ja_link: Some(pdf_ja_link),
            html_en_link,
            html_ja_link,
            bib_link,
            date,
        };
        println!("{:?}", &article);
        Some(article)
    }

    fn to_arxiv(&self) -> Option<Self> {
        // タイトルと著者が完全一致なら同じ論文とみなす
        // arxiv 版の方がリッチな情報を提供できるので，できる限り変換する
        let url = format!("http://export.arxiv.org/api/query?search_query=ti:{}&max_results=1", self.title);
        let body = reqwest::get(&url).unwrap().text().unwrap();

        let mut reader = XmlReader::from_str(&body);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut title = String::new();
        let mut authors: Vec<String> = Vec::new();
        let mut in_title = false;
        let mut in_author = false;
        let mut arxiv_link = String::new();

        loop {
            match reader.read_event(&mut buf) {
                Ok(XmlEvent::Empty(ref e)) => {
                    match e.name() {
                        b"link" => {
                            let attr = e.attributes().map(|a| a.unwrap()).find(|a| a.key == b"href").unwrap();
                            arxiv_link = String::from_utf8(attr.unescaped_value().unwrap().to_vec()).unwrap();
                        }
                        _ => (),
                    }
                }
                Ok(XmlEvent::Start(ref e)) => {
                    match e.name() {
                        b"title" => in_title = true,
                        b"author" => in_author = true,
                        _ => (),
                    }
                }
                Ok(XmlEvent::End(ref e)) => {
                    match e.name() {
                        b"title" => in_title = false,
                        b"author" => in_author = false,
                        _ => (),
                    }
                }
                Ok(XmlEvent::Text(ref e)) if in_title => title = e.unescape_and_decode(&reader).unwrap(),
                Ok(XmlEvent::Text(ref e)) if in_author => authors.push(e.unescape_and_decode(&reader).unwrap()),
                Ok(XmlEvent::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (),
            }
        }

        if title == self.title && authors == self.authors {
            println!("{}", &title);
            println!("same article on arxiv: {}", &arxiv_link);
            let arxiv_article = Article::from_arxiv(&arxiv_link).unwrap();
            let mut article: Self = self.clone();
            if article.abst.is_none() {
                article.abst = arxiv_article.abst;
            }
            if article.url_ja.is_none() {
                article.url_ja = arxiv_article.url_ja;
            }
            if article.html_en_link.is_none() {
                article.html_en_link = arxiv_article.html_en_link;
            }
            if article.html_ja_link.is_none() {
                article.html_ja_link = arxiv_article.html_ja_link;
            }
            Some(article)
        } else {
            None
        }
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

#[test]
fn test_acm() {
    let article = Article::from_acm("https://dl.acm.org/citation.cfm?id=1073465").unwrap();
    assert_eq!(article.id, "1073465".to_string());
    assert_eq!(article.title, "Automatic evaluation of summaries using N-gram co-occurrence statistics".to_string());
    assert_eq!(article.authors, vec!["Lin, Chin-Yew".to_string(), "Hovy, Eduard".to_string()]);
    assert_eq!(article.abst, None);
    assert_eq!(article.pdf_en_link, Some("http://dl.acm.org/ft_gateway.cfm?id=1073465&type=pdf".to_string()));
}

#[test]
fn test_nips() {
    let article = Article::from_nips("http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information").unwrap();
    assert_eq!(article.id, "3730".to_string());
    assert_eq!(article.title, "Streaming Pointwise Mutual Information".to_string());
    assert_eq!(article.url, "http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information".to_string());
    assert_eq!(article.authors, vec!["Benjamin V. Durme".to_string(), "Ashwin Lall".to_string()]);
    assert_eq!(article.abst, Some("Recent work has led to the ability to perform space efﬁcient, approximate counting  over large vocabularies in a streaming context. Motivated by the existence of data  structures of this type, we explore the computation of associativity scores, other-  wise known as pointwise mutual information (PMI), in a streaming context. We  give theoretical bounds showing the impracticality of perfect online PMI compu-  tation, and detail an algorithm with high expected accuracy. Experiments on news  articles show our approach gives high accuracy on real world data.".to_string()));
    assert_eq!(article.pdf_en_link, Some("http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information.pdf".to_string()));

    let article = Article::from_nips("http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information.pdf").unwrap();
    assert_eq!(article.id, "3730".to_string());
    assert_eq!(article.title, "Streaming Pointwise Mutual Information".to_string());
    assert_eq!(article.url, "http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information".to_string());
    assert_eq!(article.authors, vec!["Benjamin V. Durme".to_string(), "Ashwin Lall".to_string()]);
    assert_eq!(article.abst, Some("Recent work has led to the ability to perform space efﬁcient, approximate counting  over large vocabularies in a streaming context. Motivated by the existence of data  structures of this type, we explore the computation of associativity scores, other-  wise known as pointwise mutual information (PMI), in a streaming context. We  give theoretical bounds showing the impracticality of perfect online PMI compu-  tation, and detail an algorithm with high expected accuracy. Experiments on news  articles show our approach gives high accuracy on real world data.".to_string()));
    assert_eq!(article.pdf_en_link, Some("http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information.pdf".to_string()));

    let article = Article::from_nips("http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information/bibtex").unwrap();
    assert_eq!(article.id, "3730".to_string());
    assert_eq!(article.title, "Streaming Pointwise Mutual Information".to_string());
    assert_eq!(article.url, "http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information".to_string());
    assert_eq!(article.authors, vec!["Benjamin V. Durme".to_string(), "Ashwin Lall".to_string()]);
    assert_eq!(article.abst, Some("Recent work has led to the ability to perform space efﬁcient, approximate counting  over large vocabularies in a streaming context. Motivated by the existence of data  structures of this type, we explore the computation of associativity scores, other-  wise known as pointwise mutual information (PMI), in a streaming context. We  give theoretical bounds showing the impracticality of perfect online PMI compu-  tation, and detail an algorithm with high expected accuracy. Experiments on news  articles show our approach gives high accuracy on real world data.".to_string()));
    assert_eq!(article.pdf_en_link, Some("http://papers.nips.cc/paper/3730-streaming-pointwise-mutual-information.pdf".to_string()));
}

#[test]
fn test_pmlr() {
    let article = Article::from_pmlr("http://proceedings.mlr.press/v48/shaha16.html").unwrap();
    assert_eq!(article.id, "v48/shaha16".to_string());
    assert_eq!(article.title, "No Oops, You Won’t Do It Again: Mechanisms for Self-correction in Crowdsourcing".to_string());
    assert_eq!(article.url, "http://proceedings.mlr.press/v48/shaha16".to_string());
    assert_eq!(article.authors, vec!["Nihar Shah".to_string(), "Dengyong Zhou".to_string()]);
    assert_eq!(article.abst, Some(r#"Crowdsourcing is a very popular means of obtaining the large amounts of labeled data that modern machine learning methods require. Although cheap and fast to obtain, crowdsourced labels suffer from significant amounts of error, thereby degrading the performance of downstream machine learning tasks. With the goal of improving the quality of the labeled data, we seek to mitigate the many errors that occur due to silly mistakes or inadvertent errors by crowdsourcing workers. We propose a two-stage setting for crowdsourcing where the worker first answers the questions, and is then allowed to change her answers after looking at a (noisy) reference answer. We mathematically formulate this process and develop mechanisms to incentivize workers to act appropriately. Our mathematical guarantees show that our mechanism incentivizes the workers to answer honestly in both stages, and refrain from answering randomly in the first stage or simply copying in the second. Numerical experiments reveal a significant boost in performance that such "self-correction" can provide when using crowdsourcing to train machine learning algorithms."#.to_string()));
    assert_eq!(article.pdf_en_link, Some("http://proceedings.mlr.press/v48/shaha16.pdf".to_string()));

    let article = Article::from_pmlr("http://proceedings.mlr.press/v48/shaha16.pdf").unwrap();
    assert_eq!(article.id, "v48/shaha16".to_string());
    assert_eq!(article.title, "No Oops, You Won’t Do It Again: Mechanisms for Self-correction in Crowdsourcing".to_string());
    assert_eq!(article.url, "http://proceedings.mlr.press/v48/shaha16".to_string());
    assert_eq!(article.authors, vec!["Nihar Shah".to_string(), "Dengyong Zhou".to_string()]);
    assert_eq!(article.abst, Some(r#"Crowdsourcing is a very popular means of obtaining the large amounts of labeled data that modern machine learning methods require. Although cheap and fast to obtain, crowdsourced labels suffer from significant amounts of error, thereby degrading the performance of downstream machine learning tasks. With the goal of improving the quality of the labeled data, we seek to mitigate the many errors that occur due to silly mistakes or inadvertent errors by crowdsourcing workers. We propose a two-stage setting for crowdsourcing where the worker first answers the questions, and is then allowed to change her answers after looking at a (noisy) reference answer. We mathematically formulate this process and develop mechanisms to incentivize workers to act appropriately. Our mathematical guarantees show that our mechanism incentivizes the workers to answer honestly in both stages, and refrain from answering randomly in the first stage or simply copying in the second. Numerical experiments reveal a significant boost in performance that such "self-correction" can provide when using crowdsourcing to train machine learning algorithms."#.to_string()));
    assert_eq!(article.pdf_en_link, Some("http://proceedings.mlr.press/v48/shaha16.pdf".to_string()));
}

fn convert_google_translation_url(url: &str) -> String { format!("https://translate.google.co.jp/translate?sl=en&tl=ja&js=y&prev=_t&hl=ja&ie=UTF-8&u={}&edit-text=&act=url", &url) }

#[derive(FromForm)]
struct Auth {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct VerificationCode {
    access_token: String,
    // scope: String,
    // team_name: String,
    team_id: String,
}

#[get("/authorize?<auth>")]
fn authorize(auth: Auth) -> String {
    let url = format!("https://slack.com/api/oauth.access?code={}&client_id={}&client_secret={}", &auth.code, env::var("CLIENT_ID1").unwrap(), env::var("CLIENT_SECRET1").unwrap());
    eprintln!("code: {}", &auth.code);
    eprintln!("state: {}", &auth.state);
    eprintln!("authorization url: {}", &url);
    let json: VerificationCode = reqwest::get(&url).unwrap().json().unwrap();
    env::set_var(format!("OAUTH1_{}", &json.team_id), &json.access_token);
    json.access_token
}

#[get("/")]
fn hello() -> String {
    "hello".to_string()
}

#[post("/", format = "application/json", data = "<message>")]
fn index(message: Json<Message>) -> String {
    match message.0.challenge {
        Some(val) => return val,
        None => ()
    }

    let token = message.0.token.to_string();
    let event: Event = message.0.event.unwrap();

    thread::spawn(move || {
        for link in &event.links {
            let article = match &link.domain[..] {
                "arxiv.org" => Article::from_arxiv(&link.url),
                "openreview.net" => Article::from_openreview(&link.url),
                "aclweb.org" | "aclanthology.coli.uni-saarland.de" | "aclanthology.info" => Article::from_aclweb(&link.url),
                "dl.acm.org" | "delivery.acm.org" => Article::from_acm(&link.url),
                "papers.nips.cc" => Article::from_nips(&link.url),
                "proceedings.mlr.press" => Article::from_pmlr(&link.url),
                //"ieeexplore.ieee.org" => Article::from_ieee(&link.url),
                _ => None,
            };
            let attachment = match article {
                Some(article) => {
                    if article.preserver != "arxiv".to_string() {
                        match article.to_arxiv() {
                            Some(new_article) => Some(Attachment::new(new_article)),
                            None => Some(Attachment::new(article)),
                        }
                    } else {
                        Some(Attachment::new(article))
                    }
                }
                None => None,
            };
            match attachment {
                Some(attachment) => send_unfurl_request(&event.channel, &event.message_ts, &link.url, &token, attachment),
                None => (),
            };
        }
    });

    String::new()
}

fn send_unfurl_request(channel: &str, ts: &str, url: &str, verification_token: &str, attachment: Attachment) {
    let unfurls: HashMap<String, Attachment> = vec![
        (url.to_string(), attachment),
    ].into_iter().collect();
    let ur = UnfurlRequest { channel: channel.to_string(), ts: ts.to_string(), unfurls };

    let client = reqwest::Client::new();
    let mut res = client.post("https://slack.com/api/chat.unfurl")
        .header(Authorization(Bearer {
            token: TOKEN_TO_OAUTH.get(verification_token).unwrap().to_string()
        }))
        .json(&ur)
        .send().ok().unwrap();
    let content = res.text().unwrap();
    println!("{}", content);
}

lazy_static! {
    static ref TOKEN_TO_OAUTH: HashMap<String, String> = {
        let mut m = HashMap::new();
        m.insert(env::var("TOKEN1").unwrap(), env::var("OAUTH1").unwrap());
        m.insert(env::var("TOKEN2").unwrap(), env::var("OAUTH2").unwrap());
        m
    };
}

fn main() {
    rocket::ignite().mount("/", routes![index, hello, authorize]).launch();
}
