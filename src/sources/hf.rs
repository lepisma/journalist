use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Paper {
    pub id: String,
    pub title: String,
    pub link: String,
    pub description: String,
    pub tags: Vec<String>,
    pub arxiv: Option<String>,
    pub added: DateTime<Utc>,
}

pub fn read_papers() -> Vec<Paper> {
    vec![]
}
