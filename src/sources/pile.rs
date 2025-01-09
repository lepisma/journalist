use std::{path, fs::File};
use std::io::{self, BufRead};
use chrono::NaiveDateTime;
use regex::Regex;
use anyhow::{Result, anyhow, Context};

#[derive(Debug)]
pub struct Bookmark {
    pub id: String,
    pub link: String,
    pub title: String,
    pub tags: Vec<String>,
    pub created: chrono::NaiveDateTime,
}

impl Bookmark {
    pub fn is_unread(&self) -> bool {
        self.tags.contains(&"unsorted".to_string())
    }
}

// Read #+TAGS: from the file and return a list
// This doesn't read filetags like it should
fn read_tags(file_path: &path::Path) -> Vec<String> {
    let tag_regex = Regex::new(r"(?i)^\#\+TAGS:\s*(.*)").unwrap();

    if let Ok(file) = File::open(file_path) {
        for line in io::BufReader::new(file).lines() {
            if let Ok(line_content) = line {
                if let Some(captures) = tag_regex.captures(&line_content) {
                    if let Some(tags) = captures.get(1) {
                        return tags.as_str()
                            .split(",")
                            .map(|tag| tag.trim().to_string())
                            .collect();
                    }
                }
            }
        }
    }
    Vec::new()
}

// Read datetime of creation of the file
fn read_datetime(file_path: &path::Path) -> Result<chrono::NaiveDateTime> {
    let file_name = file_path
        .file_name()
        .context("Not able to get file name")?
        .to_str()
        .context("Failed to convert file name to str")?;

    // Files are named in the following pattern
    // YYYYmmddHHMMSS-<stuff>.org
    if let Some((first, _)) = file_name.to_string().split_once("-") {
        Ok(chrono::NaiveDateTime::parse_from_str(first, "%Y%m%d%H%M%S")?)
    } else {
        Err(anyhow!("Error in parsing file: {}", file_name))
    }
}

// Read bookmarks from my org-roam base
//
// Any file that's in the literature subdir and has `unsorted` (or no) tag is a
// bookmark to consider.
pub fn read_bookmarks(roam_db_path: &path::Path) -> Vec<Bookmark> {
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
        let file_path_str = statement.read::<String, _>("file").unwrap();
        let file_path = path::Path::new(&file_path_str);

        output.push(Bookmark {
            id: statement.read::<String, _>("id").unwrap(),
            link: statement.read::<String, _>("ref").unwrap(),
            title: statement.read::<String, _>("title").unwrap(),
            tags: read_tags(file_path),
            created: read_datetime(file_path).unwrap_or(chrono::Local::now().naive_local()),
        });
    }

    output
}
