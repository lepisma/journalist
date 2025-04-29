use chrono::{DateTime, Datelike, Utc};
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header;
use scraper::{Html, Selector};

use crate::{NewsItem, ToNewsItem};

#[derive(Debug, Clone)]
pub struct Paper {
    id: String,
    title: String,
    link: String,
    description: String,
    tags: Vec<String>,
    arxiv: Option<String>,
    added: DateTime<Utc>,
    votes: usize,
    n_comments: usize,
}

#[derive(Debug)]
pub struct Week {
    year: usize,
    week: usize,
}

impl ToNewsItem for Paper {
    fn to_newsitem(&self) -> NewsItem {
        NewsItem {
            id: self.id.clone(),
            link: self.link.clone(),
            title: self.title.clone(),
            summary: if self.description.is_empty() { None } else { Some(self.description.clone()) },
            published: self.added,
            updated: self.added,
            authors: Vec::new(),
            categories: self.tags.clone(),
        }
    }
}

pub fn get_current_week() -> Week {
    let now = chrono::Local::now();
    let year = now.year() as usize;
    let week = now.iso_week().week() as usize;

    Week { year, week }
}

pub fn read_weekly_papers(week: Week) -> Result<Vec<Paper>> {
    let url = format!("https://huggingface.co/papers/week/{}-W{}", week.year, week.week);

    let mut headers = header::HeaderMap::new();
    headers.insert("Accept", header::HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"));
    headers.insert("Accept-Language", header::HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert("Cache-Control", header::HeaderValue::from_static("no-cache"));
    headers.insert("Pragma", header::HeaderValue::from_static("no-cache"));

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
        .default_headers(headers)
        .build()?;

    let response = client.get(&url).send()?;
    let body = response.text()?;
    let document = Html::parse_document(&body);

    let selector = Selector::parse("div.\\[content-visibility\\:auto\\] > article:nth-child(1) > div:nth-child(3) > div:nth-child(1)").unwrap();
    let vote_selector = &Selector::parse("div:nth-child(1)").unwrap();
    let title_selector = &Selector::parse("div:nth-child(2) > h3:nth-child(1) > a:nth-child(1)").unwrap();

    let mut papers = Vec::new();

    for element in document.select(&selector) {
        let vote_element = element.select(&vote_selector).next().unwrap();
        let votes: usize = vote_element.text().collect::<String>().trim().parse().unwrap();

        let title_element = element.select(&title_selector).next().unwrap();
        let title = title_element.text().collect::<String>().trim().to_string();
        let rel_link = title_element.attr("href").unwrap().to_string();

        let paper = Paper {
            id: rel_link.clone(),
            title,
            link: format!("https://huggingface.co{}", rel_link),
            description: "".to_string(),
            tags: vec![],
            arxiv: None,
            added: Utc::now(),
            votes,
            n_comments: 0,
        };

        papers.push(paper);
    }

    Ok(papers)
}
