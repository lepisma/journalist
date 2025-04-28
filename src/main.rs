use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use log::debug;
use std::{fs::File, io::Write, ops::Add, path};
use anyhow::{anyhow, Result};
use sources::{hf, pile};
use rand::seq::SliceRandom;
use htmlescape::encode_minimal;

mod sources;
mod utils;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Generate {
        #[command(subcommand)]
        gen_command: GenCommands,
    },
    Merge {
        #[arg(long)]
        input: Vec<path::PathBuf>,
        output_file: path::PathBuf,
    },
}

#[derive(Subcommand)]
enum GenCommands {
    PileBookmarks {
        #[arg(long)]
        roam_db_path: Option<path::PathBuf>,
        #[arg(long)]
        notes_dir_path: Option<path::PathBuf>,
        output_file: path::PathBuf,
    },
    PileBookmarksProjects {
        #[arg(long)]
        roam_db_path: Option<path::PathBuf>,
        #[arg(long)]
        notes_dir_path: Option<path::PathBuf>,
        output_file: path::PathBuf,
    },
    HfPapers {
        output_file: path::PathBuf,
    },
    RecommendedLinks {
        #[arg(long)]
        roam_db_path: Option<path::PathBuf>,
        #[arg(long)]
        notes_dir_path: Option<path::PathBuf>,
        output_file: path::PathBuf,
    },
}

#[derive(Clone, serde::Serialize, Debug)]
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

#[derive(Clone, serde::Serialize, Debug)]
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

impl Add for NewsItem {
    type Output = Result<Self>;

    fn add(self, other: Self) -> Result<Self> {
        if self.id != other.id {
            Err(anyhow!("{:?} and {:?} have different IDs", self, other))
        } else {
            let item = NewsItem {
                id: self.id,
                link: self.link,
                title: self.title,
                summary: if self.summary.is_some() {
                    if other.summary.is_some() {
                        Some(format!("{}\n-----\n{}", self.summary.unwrap(), other.summary.unwrap()))
                    } else {
                        self.summary
                    }
                } else {
                    other.summary
                },
                published: self.published,
                updated: std::cmp::max(self.updated, other.updated),
                authors: self.authors,
                categories: utils::union_strings(self.categories, other.categories),
            };
            Ok(item)
        }
    }
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
            summary: self.content.clone(),
            // NOTE: This is semantically wrong since created (when bookmark was
            //       saved) != published (when content was actually published).
            published: self.created,
            updated: self.created,
            authors: Vec::new(),
            categories: self.tags.clone(),
        }
    }
}

impl ToNewsItem for hf::Paper {
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
  <summary type="text">{{ item.summary }}</summary>
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
    env_logger::init();

    let author: NewsAuthor = NewsAuthor {
        name: "Abhinav Tushar".to_string(),
        email: "abhinav@lepisma.xyz".to_string(),
        uri: "lepisma.xyz".to_string(),
    };


    match args.command {
        Commands::Merge { input: _, output_file: _ } => {
            return Err(anyhow!("Merge operation not implemented yet!"));
        },
        Commands::Generate { gen_command } => {
            let bookmarks: Vec<_>;
            let feed: NewsFeed;

            match gen_command {
                GenCommands::PileBookmarks { roam_db_path, notes_dir_path, output_file } => {
                    if let Some(db_path) = roam_db_path {
                        bookmarks = pile::read_bookmarks(db_path.as_path());
                    } else if let Some(dir_path) = notes_dir_path {
                        bookmarks = pile::read_bookmarks_from_dir(dir_path.as_path());
                    } else {
                        panic!("Need either --notes-dir-path or --roam-db-path to be set!");
                    }

                    let mut general_bookmarks: Vec<_> = bookmarks
                        .iter()
                        .filter(|bm| bm.is_unread())
                        .filter(|bm| !bm.is_project())
                        .collect();

                    general_bookmarks.shuffle(&mut rng);

                    feed = NewsFeed {
                        id: "pile-bookmarks".to_string(),
                        title: "General Bookmarks".to_string(),
                        items: general_bookmarks.iter().map(|bm| bm.to_newsitem()).take(2).collect(),
                        authors: vec![author.clone()],
                        categories: Vec::new(),
                        generator: "journalist".to_string(),
                        link: "/pile-bookmarks".to_string(),
                        updated: Utc::now(),
                        subtitle: "Unread picks from saved bookmarks.".to_string(),
                    };

                    let mut feed_file = File::create(output_file)?;
                    feed_file.write_all(feed.to_xml_string().as_bytes())?;
                },
                GenCommands::PileBookmarksProjects { roam_db_path, notes_dir_path, output_file } => {
                    if let Some(db_path) = roam_db_path {
                        bookmarks = pile::read_bookmarks(db_path.as_path());
                    } else if let Some(dir_path) = notes_dir_path {
                        bookmarks = pile::read_bookmarks_from_dir(dir_path.as_path());
                    } else {
                        panic!("Need either --notes-dir-path or --roam-db-path to be set!");
                    }

                    let mut project_bookmarks: Vec<_> = bookmarks
                        .iter()
                        .filter(|bm| bm.is_unread())
                        .filter(|bm| bm.is_project())
                        .collect();

                    project_bookmarks.shuffle(&mut rng);

                    feed = NewsFeed {
                        id: "pile-bookmarks-projects".to_string(),
                        title: "Unsorted Projects".to_string(),
                        items: project_bookmarks.iter().map(|bm| bm.to_newsitem()).take(1).collect(),
                        authors: vec![author.clone()],
                        categories: Vec::new(),
                        generator: "journalist".to_string(),
                        link: "/pile-bookmarks-projects".to_string(),
                        updated: Utc::now(),
                        subtitle: "Unsorted projects from saved bookmarks.".to_string(),
                    };

                    let mut feed_file = File::create(output_file)?;
                    feed_file.write_all(feed.to_xml_string().as_bytes())?;
                },
                GenCommands::HfPapers { output_file: _ } => {
                    return Err(anyhow!("HF Papers feed generator is not ready yet!"));
                },
                GenCommands::RecommendedLinks { roam_db_path, notes_dir_path, output_file } => {
                    if let Some(db_path) = roam_db_path {
                        bookmarks = pile::read_bookmarks(db_path.as_path());
                    } else if let Some(dir_path) = notes_dir_path {
                        bookmarks = pile::read_bookmarks_from_dir(dir_path.as_path());
                    } else {
                        panic!("Need either --notes-dir-path or --roam-db-path to be set!");
                    }

                    let recommended_bookmarks: Vec<_> = bookmarks
                        .iter()
                        .filter(|bm| bm.is_recommended())
                        .collect();
                    feed = NewsFeed {
                        id: "recommended-links".to_string(),
                        title: "lepisma's recommended links".to_string(),
                        items: recommended_bookmarks.iter().map(|bm| bm.to_newsitem()).collect(),
                        authors: vec![author.clone()],
                        categories: Vec::new(),
                        generator: "journalist".to_string(),
                        link: "/recommended-links".to_string(),
                        updated: Utc::now(),
                        subtitle: "Recommendations from lepisma's list of read articles and bookmarks".to_string()
                    };

                    let mut feed_file = File::create(output_file)?;
                    feed_file.write_all(feed.to_xml_string().as_bytes())?;
                }
            }
        }
    }

    Ok(())
}
