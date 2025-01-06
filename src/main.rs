use chrono::{DateTime, Utc};
use clap::Parser;
use std::{fs::File, io::Write, path};
use anyhow::Result;
use sources::pile;
use rand::seq::SliceRandom;

mod sources;

#[derive(Parser)]
struct Cli {
    output_path: path::PathBuf,
    roam_db_path: path::PathBuf,
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
            updated: Utc::now(),
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
        context.insert("item", &self);
        context.insert("authors", &self.authors.clone().into_iter().map(|a| a.to_xml_string()).collect::<Vec<_>>());
        tera.render("news-item", &context).unwrap()
    }
}

fn format_opml_string(feeds: Vec<NewsFeed>) -> String {
    "TESTING".to_string()

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

    let mut unread_bookmarks: Vec<_> = pile::read_bookmarks(args.roam_db_path.as_path())
        .into_iter()
        .filter(|bm| bm.is_unread())
        .collect();
    unread_bookmarks.shuffle(&mut rng);

    let items: Vec<NewsItem> = unread_bookmarks.into_iter().take(5).map(|bm| bm.to_newsitem()).collect();
    let feeds: Vec<NewsFeed> = vec![NewsFeed {
        id: "/pile-bookmarks".to_string(),
        title: "Bookmarks".to_string(),
        items,
        authors: vec![NewsAuthor {
            name: "Abhinav Tushar".to_string(),
            email: "abhinav@lepisma.xyz".to_string(),
            uri: "lepisma.xyz".to_string(),
        }],
        categories: Vec::new(),
        generator: "journalist".to_string(),
        link: "/pile-bookmarks".to_string(),
        updated: Utc::now(),
        subtitle: "Unread picks from the saved bookmarks.".to_string(),
    }];

    for feed in &feeds {
        let feed_file_path = args.output_path.join(feed.title.to_lowercase() + ".xml");
        let mut feed_file = File::create(feed_file_path)?;
        feed_file.write_all(feed.to_xml_string().as_bytes())?;
    }

    let opml_string = format_opml_string(feeds);
    let opml_file_path = args.output_path.join("journalist.opml");

    let mut opml_file = File::create(opml_file_path)?;
    opml_file.write_all(opml_string.as_bytes())?;

    Ok(())
}
