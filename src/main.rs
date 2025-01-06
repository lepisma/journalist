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

struct NewsFeed {
    title: String,
    items: Vec<NewsItem>,
}

#[derive(Clone)]
struct NewsItem {
    link: String,
    title: String,
    description: String,
    date: DateTime<Utc>,
    categories: Vec<String>,
}

trait ToNewsItem {
    fn to_newsitem(&self) -> NewsItem;
}

impl ToNewsItem for pile::Bookmark {
    fn to_newsitem(&self) -> NewsItem {
        NewsItem {
            link: self.link.clone(),
            title: self.title.clone(),
            description: "NA".to_string(),
            date: Utc::now(),
            categories: self.tags.clone(),
        }
    }
}

fn format_opml_string(feeds: Vec<NewsFeed>) -> String {
    "TESTING".to_string()
}

fn format_newsfeed(feed: &NewsFeed) -> String {
    format!(r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>{}</title>
  <id>{}</id>
  <updated>{}</updated>

{}
</feed>"#,
            feed.title,
            "TODO",
            Utc::now().to_rfc3339(),
            feed.items.clone().into_iter().map(|it| format_newsitem(&it)).collect::<Vec<String>>().join("\n"))
}

fn format_newsitem(item: &NewsItem) -> String {
    format!(r#"<entry>
  <title>{}</title>
  <link href="{}" />
  <id>urn:uuid:{}</id>
  <updated>{}</updated>
  <summary>{}</summary>
</entry>"#,
            item.title,
            item.link,
            uuid::Uuid::new_v4(),
            item.date.to_rfc3339(),
            item.description)
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
        title: "Bookmarks".to_string(),
        items
    }];

    for feed in &feeds {
        let feed_file_path = args.output_path.join(feed.title.to_lowercase() + ".xml");
        let mut feed_file = File::create(feed_file_path)?;
        feed_file.write_all(format_newsfeed(feed).as_bytes())?;
    }

    let opml_string = format_opml_string(feeds);
    let opml_file_path = args.output_path.join("journalist.opml");

    let mut opml_file = File::create(opml_file_path)?;
    opml_file.write_all(opml_string.as_bytes())?;

    Ok(())
}
