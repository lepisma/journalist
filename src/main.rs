use chrono::{DateTime, Utc};
use clap::Parser;
use std::{fs::File, io::Write, path};
use anyhow::Result;
use sources::{hf::{self, read_papers}, pile};
use rand::seq::SliceRandom;
use htmlescape::encode_minimal;

mod sources;

#[derive(Parser)]
struct Cli {
    output_path: path::PathBuf,

    #[arg(long)]
    roam_db_path: Option<path::PathBuf>,

    #[arg(long)]
    notes_dir_path: Option<path::PathBuf>,
}

#[derive(Clone, serde::Serialize)]
struct NewsAuthor {
    name: String,
    email: String,
    uri: String,
}

#[derive(serde::Serialize)]
struct NewsFeed {
    id: String,
    updated: DateTime<Utc>,
    link: String,
    title: String,
    subtitle: String,
    items: Vec<NewsItem>,
    authors: Vec<NewsAuthor>,
    categories: Vec<String>,
    generator: String
}

#[derive(Clone, serde::Serialize)]
struct NewsItem {
    id: String,
    link: String,
    title: String,
    summary: Option<String>,
    published: DateTime<Utc>,
    updated: DateTime<Utc>,
    authors: Vec<NewsAuthor>,
    categories: Vec<String>,
}

trait ToNewsItem {
    fn to_newsitem(&self) -> NewsItem;
}

trait ToXmlString {
    fn to_xml_string(&self) -> String;
}

impl ToNewsItem for pile::Bookmark {
    fn to_newsitem(&self) -> NewsItem {
        NewsItem {
            id: self.id.clone(),
            link: self.link.clone(),
            title: self.title.clone(),
            summary: None,
            // NOTE: This is semantically wrong since created (when bookmark was
            //       saved) != published (when content was actually published).
            published: self.created,
            updated: self.created,
            authors: Vec::new(),
            categories: self.tags.clone(),
        }
    }
}

impl ToXmlString for NewsAuthor {
    fn to_xml_string(&self) -> String {
        format!(r#"<author>
  <name>{}</name>
  <email>{}</email>
  <uri>{}</uri>
</author>"#,
                self.name,
                self.email,
                self.uri)
    }
}

impl ToXmlString for NewsItem {
    fn to_xml_string(&self) -> String {
        let template = r#"<entry>
  <title>{{ item.title }}</title>
  <link href="{{ item.link }}" />
  <id>urn:uuid:{{ item.id }}</id>
  <updated>{{ item.updated }}</updated>
  <published>{{ item.published }}</published>
  {%- if item.summary %}
  <summary>{{ item.summary }}</summary>
  {%- endif %}
  {%- for category in item.categories %}
  <category term="{{ category }}" />
  {%- endfor %}
  {%- for author in authors %}
  {{ author }}
  {%- endfor %}
</entry>"#;
        let mut tera = tera::Tera::default();
        tera.add_raw_template("news-item", template).unwrap();
        let mut context = tera::Context::new();
        context.insert("item", &NewsItem {
            id: self.id.clone(),
            title: encode_minimal(&self.title),
            link: self.link.clone(),
            published: self.published,
            updated: self.updated,
            summary: self.summary.as_ref().map(|s| encode_minimal(s)),
            categories: self.categories.clone(),
            authors: self.authors.clone(),
        });
        context.insert("authors", &self.authors.clone().into_iter().map(|a| a.to_xml_string()).collect::<Vec<_>>());
        tera.render("news-item", &context).unwrap()
    }
}

impl ToXmlString for NewsFeed {
    fn to_xml_string(&self) -> String {
        let template = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>{{ item.id }}</id>
  <title>{{ item.title }}</title>
  <subtitle>{{ item.subtitle }}</subtitle>
  <updated>{{ item.updated }}</updated>
  <link rel="self" href="{{ item.link }}" />
  {%- for category in item.categories %}
  <category term="{{ category }}" />
  {%- endfor %}
  {%- for author in authors %}
  {{ author }}
  {%- endfor %}
  <generator>{{ item.generator }}</generator>
{%- for entry in entries %}
{{ entry }}
{%- endfor %}
</feed>"#;
        let mut tera = tera::Tera::default();
        tera.add_raw_template("news-feed", template).unwrap();
        let mut context = tera::Context::new();
        context.insert("item", &self);
        context.insert("authors", &self.authors.clone().into_iter().map(|a| a.to_xml_string()).collect::<Vec<_>>());
        context.insert("entries", &self.items.clone().into_iter().map(|it| it.to_xml_string()).collect::<Vec<_>>());
        tera.render("news-feed", &context).unwrap()
    }
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let mut rng = rand::thread_rng();

    let author: NewsAuthor = NewsAuthor {
        name: "Abhinav Tushar".to_string(),
        email: "abhinav@lepisma.xyz".to_string(),
        uri: "lepisma.xyz".to_string(),
    };

    let bookmarks: Vec<_>;

    if let Some(db_path) = args.roam_db_path {
        bookmarks = pile::read_bookmarks(db_path.as_path());
    } else if let Some(dir_path) = args.notes_dir_path {
        bookmarks = pile::read_bookmarks_from_dir(dir_path.as_path());
    } else {
        panic!("Need either --notes-dir-path or --roam-db-path to be set!");
    }

    let mut unread_bookmarks: Vec<_> = bookmarks
        .into_iter()
        .filter(|bm| bm.is_unread())
        .collect();
    unread_bookmarks.shuffle(&mut rng);
    unread_bookmarks = unread_bookmarks.clone().into_iter().take(5).collect();

    let (project_items, general_items): (Vec<_>, Vec<_>) = unread_bookmarks
        .clone()
        .into_iter()
        .partition(|bm| bm.is_project());
    let project_items: Vec<NewsItem> = project_items.into_iter().map(|bm| bm.to_newsitem()).collect();
    let general_items: Vec<NewsItem> = general_items.into_iter().map(|bm| bm.to_newsitem()).collect();

    let feeds: Vec<NewsFeed> = vec![
        NewsFeed {
            id: "pile-bookmarks".to_string(),
            title: "General Bookmarks".to_string(),
            items: general_items,
            authors: vec![author.clone()],
            categories: Vec::new(),
            generator: "journalist".to_string(),
            link: "/pile-bookmarks".to_string(),
            updated: Utc::now(),
            subtitle: "Unread picks from saved bookmarks.".to_string(),
        },
        NewsFeed {
            id: "pile-bookmarks-projects".to_string(),
            title: "Unsorted Projects".to_string(),
            items: project_items,
            authors: vec![author.clone()],
            categories: Vec::new(),
            generator: "journalist".to_string(),
            link: "/pile-bookmarks-projects".to_string(),
            updated: Utc::now(),
            subtitle: "Unsorted projects from saved bookmarks.".to_string(),
        },
    ];

    for feed in &feeds {
        let feed_file_path = args.output_path.join(feed.id.clone() + ".xml");
        let mut feed_file = File::create(feed_file_path)?;
        feed_file.write_all(feed.to_xml_string().as_bytes())?;
    }

    Ok(())
}
