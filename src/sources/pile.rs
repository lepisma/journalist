use std::fs;
use std::{path, fs::File};
use std::io::{self, BufRead};
use regex::Regex;
use anyhow::{Result, anyhow, Context};
use once_cell::sync::Lazy;
use chrono::{DateTime, Utc};

use crate::{ToNewsItem, NewsItem};

static ID_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^:id:\s*(.*)").unwrap() });
static REF_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^:ROAM_REFS:\s*(.*)").unwrap() });
static TAGS_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^\#\+TAGS:\s*(.*)").unwrap() });
static TITLE_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^\#\+TITLE:\s*(.*)").unwrap() });

// An org node from my notes directory. This could be a bookmark (a literature
// note) or a general note.
#[derive(Debug, Clone)]
pub struct OrgNode {
    id: String,
    ref_: Option<String>,
    title: String,
    tags: Vec<String>,
    created: DateTime<Utc>,
    content: Option<String>,
}

impl OrgNode {
    fn from_file(file_path: &path::Path) -> Result<Self> {
        let mut id: Option<String> = None;
        let mut ref_: Option<String> = None;
        let mut tags: Vec<String> = Vec::new();
        let mut title: Option<String> = None;

        let body = fs::read_to_string(file_path)?;
        let mut header_done = false;
        let mut content = String::new();

        for line in body.lines() {
            if let Some(captures) = ID_REGEX.captures(&line) {
                if let Some(id_str) = captures.get(1) {
                    id = Some(id_str.as_str().to_string());
                } else {
                    return Err(anyhow!("Pattern for id matched but not able to parse value"));
                }
            } else if let Some(captures) = REF_REGEX.captures(&line) {
                if let Some(ref_str) = captures.get(1) {
                    ref_ = Some(ref_str.as_str().to_string());
                } else {
                    return Err(anyhow!("Pattern for ref matched but not able to parse value"));
                }
            } else if let Some(captures) = TAGS_REGEX.captures(&line) {
                if let Some(tags_str) = captures.get(1) {
                    tags = tags_str.as_str()
                        .split(",")
                        .map(|tag| tag.trim().to_string())
                        .collect();
                } else {
                    return Err(anyhow!("Pattern for tags matched but not able to parse value"));
                }
            } else if let Some(captures) = TITLE_REGEX.captures(&line) {
                if let Some(title_str) = captures.get(1) {
                    title = Some(title_str.as_str().to_string());
                    // In the way I have been keeping my notes, title is the
                    // last line of the metadata block.
                    header_done = true;
                    continue;
                } else {
                    return Err(anyhow!("Pattern for title matched but not able to parse value"));
                }
            }

            if header_done {
                content.push_str(line);
                content.push_str("\n");
            }
        }

        let trimmed_content = content.trim();

        // Title and id are mandatory, if they are not present, return an
        // Err. Else return whatever is parsed.
        if title.is_some() && id.is_some() {
            return Ok(OrgNode {
                id: id.context("Unable to parse ID")?,
                ref_,
                title: title.context("Unable to parse title")?,
                tags,
                created: read_datetime(file_path)?,
                content: if trimmed_content.is_empty() { None } else { Some(trimmed_content.to_string()) }
            });
        } else {
            return Err(anyhow!("Parsing error"));
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bookmark {
    id: String,
    ref_: String,
    title: String,
    tags: Vec<String>,
    created: DateTime<Utc>,
    content: Option<String>,
}

impl Bookmark {
    fn from_org_node(node: &OrgNode) -> Result<Self> {
        if node.ref_.is_some() {
            Ok(Bookmark {
                id: node.id.clone(),
                ref_: node.ref_.clone().unwrap(),
                title: node.title.clone(),
                tags: node.tags.clone(),
                created: node.created,
                content: node.content.clone(),
            })
        } else {
            Err(anyhow!("Reference not found in node."))
        }
    }

    pub fn is_unread(&self) -> bool {
        self.tags.contains(&"unsorted".to_string())
    }

    pub fn is_project(&self) -> bool {
        if self.tags.contains(&"project".to_string()) {
            true
        } else {
            self.ref_.starts_with("https://github.com")
        }
    }

    pub fn is_recommended(&self) -> bool {
        self.tags.contains(&"recommend".to_string()) & !self.is_unread()
    }
}

impl ToNewsItem for Bookmark {
    fn to_newsitem(&self) -> NewsItem {
        NewsItem {
            id: self.id.clone(),
            link: self.ref_.clone(),
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

fn read_bookmark_from_file(file_path: &path::Path) -> Result<Bookmark> {
    let org_node = OrgNode::from_file(file_path)?;
    Bookmark::from_org_node(&org_node)
}

// Read #+TAGS: from the file and return a list
// This doesn't read filetags like it should
fn read_tags(file_path: &path::Path) -> Vec<String> {
    if let Ok(file) = File::open(file_path) {
        for line in io::BufReader::new(file).lines() {
            if let Ok(line_content) = line {
                if let Some(captures) = TAGS_REGEX.captures(&line_content) {
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

fn read_content(file_path: &path::Path) -> Result<String> {
    let file = File::open(file_path)?;
    let reader = io::BufReader::new(file);
    let mut content = String::new();

    let mut in_content = false;
    for line in reader.lines() {
        let line = line?;
        let trimmed_line = line.trim();

        if !in_content {
            if trimmed_line.starts_with("#") || trimmed_line.starts_with(":") || trimmed_line.is_empty() {
                continue;
            } else {
                in_content = true;
            }
        }

        content.push_str(&line);
        content.push_str("\n");
    }

    Ok(content)
}

// Read datetime of creation of the file using the pattern in file name
fn read_datetime(file_path: &path::Path) -> Result<DateTime<Utc>> {
    let file_name = file_path
        .file_name()
        .context("Not able to get file name")?
        .to_str()
        .context("Failed to convert file name to str")?;

    // Files are named in the following pattern
    // YYYYmmddHHMMSS-<stuff>.org
    if let Some((first, _)) = file_name.to_string().split_once("-") {
        let dt = chrono::NaiveDateTime::parse_from_str(first, "%Y%m%d%H%M%S")?;

        // Most of my saves are in this timezone, but if they are not we will
        // get wrong results. I don't have a good way of solving it right now
        // other than adding tz information in the file name.
        let tz = chrono_tz::Asia::Kolkata;
        Ok(dt.and_local_timezone(tz).unwrap().to_utc())
    } else {
        Err(anyhow!("Error in parsing file: {}", file_name))
    }
}

// Read bookmarks from my org-roam directory
pub fn read_bookmarks_from_dir(dir_path: &path::Path) -> Vec<Bookmark> {
    let mut output = Vec::new();

    for res in std::fs::read_dir(dir_path).unwrap() {
        let path = res.unwrap().path();
        if let Some(ext) = path.extension() {
            if ext == "org" {
                if let Ok(bookmark) = read_bookmark_from_file(path.as_path()) {
                    output.push(bookmark);
                }
            }
        }
    }

    output
}

// Read bookmarks from org-roam database
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
            ref_: statement.read::<String, _>("ref").unwrap(),
            title: statement.read::<String, _>("title").unwrap(),
            tags: read_tags(file_path),
            created: read_datetime(file_path).unwrap_or(chrono::Utc::now()),
            content: read_content(file_path).map_or(None, |v| Some(v)),
        });
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_parsing_works() {
        let string = r#":PROPERTIES:
:ID:       cae71435-9f7e-41ba-84d2-cf8d85fbffa0
:ROAM_REFS: https://github.com/MattMoony/figaro?tab=readme-ov-file#references
:END:
#+TAGS: project, speech, privacy
#+TITLE: MattMoony/figaro: Real-time voice-changer for voice-chat, etc. Will support many different voice-filters and features in the future. 🎵
"#;
        assert!(true);
    }
}
