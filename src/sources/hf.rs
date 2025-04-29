use chrono::{DateTime, Utc};

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
}

impl ToNewsItem for Paper {
    fn to_newsitem(&self) -> NewsItem {
        NewsItem {
            id: self.id.clone(),
            link: self.link.clone(),
            title: self.title.clone(),
            summary: Some(self.description.clone()),
            published: self.added,
            updated: self.added,
            authors: Vec::new(),
            categories: self.tags.clone(),
        }
    }
}

pub fn read_papers() -> Vec<Paper> {
    vec![]
}
