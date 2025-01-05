use chrono::{DateTime, Utc};
use clap::Parser;
use std::{any::Any, fs::File, io::Write, path};
use anyhow::Result;

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

#[derive(Debug)]
struct Bookmark {
    id: String,
    link: String,
    title: String,
    tags: Vec<String>,
}

impl ToNewsItem for Bookmark {
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

// fn read_pile_datetime(file_path: &path::Path) -> DateTime<Utc> {
// }

// fn read_pile_tags(file_path: &path::Path) -> Vec<String> {
// }

// Read bookmarks from my org-roam base
//
// Any file that's in the literature subdir and has `unsorted` (or no) tag is a
// bookmark to consider.
fn read_pile_bookmarks(roam_db_path: &path::Path) -> Vec<Bookmark> {
    let connection = sqlite::open(roam_db_path).unwrap();
    let query = r#"
        SELECT
            TRIM(id, '"') AS id,
            TRIM(file, '"') AS file,
            TRIM(title, '"') AS title,
            CONCAT(TRIM(type, '"'), ':', TRIM(ref, '"')) AS ref
        FROM nodes
        INNER JOIN refs ON nodes.id = refs.node_id;"#;

    let mut output: Vec<Bookmark> = Vec::new();
    let mut statement = connection.prepare(query).unwrap();

    while let Ok(sqlite::State::Row) = statement.next() {
        output.push(Bookmark {
            id: statement.read::<String, _>("id").unwrap(),
            link: statement.read::<String, _>("ref").unwrap(),
            title: statement.read::<String, _>("title").unwrap(),
            tags: vec![],
        });
    }

    output
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

    // Pile bookmarks
    let bookmarks = read_pile_bookmarks(args.roam_db_path.as_path());

    let items: Vec<NewsItem> = bookmarks.into_iter().map(|bm| bm.to_newsitem()).collect();
    let feeds: Vec<NewsFeed> = vec![NewsFeed {
        title: "Bookmarks".to_string(),
        items
    }];

    // Write each feed to separate file
    for feed in &feeds {
        let feed_file_path = args.output_path.join(feed.title.to_lowercase() + ".xml");
        let mut feed_file = File::create(feed_file_path)?;
        feed_file.write_all(format_newsfeed(feed).as_bytes())?;
    }

    // Generate OPML
    let opml_string = format_opml_string(feeds);
    let opml_file_path = args.output_path.join("journalist.opml");

    let mut opml_file = File::create(opml_file_path)?;
    opml_file.write_all(opml_string.as_bytes())?;

    Ok(())
}
