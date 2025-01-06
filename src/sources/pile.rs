use std::{path, fs::File};
use std::io::{self, BufRead};
use regex::Regex;

#[derive(Debug)]
pub struct Bookmark {
    pub id: String,
    pub link: String,
    pub title: String,
    pub tags: Vec<String>,
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
        let file_path = statement.read::<String, _>("file").unwrap();

        output.push(Bookmark {
            id: statement.read::<String, _>("id").unwrap(),
            link: statement.read::<String, _>("ref").unwrap(),
            title: statement.read::<String, _>("title").unwrap(),
            tags: read_tags(path::Path::new(&file_path)),
        });
    }

    output
}
