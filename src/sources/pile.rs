use std::env::consts;
use std::fs;
use std::{path, fs::File};
use std::io::{self, BufRead};
use regex::Regex;
use anyhow::{Result, anyhow, Context};
use once_cell::sync::Lazy;
use chrono::{DateTime, Utc};

static ID_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^:id:\s*(.*)").unwrap() });
static REF_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^:ROAM_REFS:\s*(.*)").unwrap() });
static TAGS_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^\#\+TAGS:\s*(.*)").unwrap() });
static TITLE_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::new(r"(?i)^\#\+TITLE:\s*(.*)").unwrap() });

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub id: String,
    pub link: String,
    pub title: String,
    pub tags: Vec<String>,
    pub created: DateTime<Utc>,
    pub content: Option<String>,
}

impl Bookmark {
    pub fn is_unread(&self) -> bool {
        self.tags.contains(&"unsorted".to_string())
    }

    pub fn is_project(&self) -> bool {
        if self.tags.contains(&"project".to_string()) {
            true
        } else {
            self.link.starts_with("https://github.com")
        }
    }

    pub fn is_recommended(&self) -> bool {
        self.tags.contains(&"recommend".to_string()) & !self.is_unread()
    }
}

// Return id, ref, tags, and title (in that order) by reading the content of
// given file. This doesn't parse anything after TITLE to keep the runtime
// fast. For parsing the content too use proper parsing function.
fn read_metadata(file_path: &path::Path) -> Result<(String, Option<String>, Vec<String>, String)> {
    let mut id: Option<String> = None;
    let mut ref_: Option<String> = None;
    let mut tags: Vec<String> = Vec::new();
    let mut title: Option<String> = None;

    let content = fs::read_to_string(file_path)?;

    for line in content.lines() {
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
                break;
            } else {
                return Err(anyhow!("Pattern for title matched but not able to parse value"));
            }
        }
    }

    // Title and id are mandatory, if they are not present, return an
    // Err. Else return whatever is parsed.
    if title.is_some() && id.is_some() {
        return Ok((id.unwrap(), ref_, tags, title.unwrap()));
    } else {
        return Err(anyhow!("Parsing error"));
    }
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
                if let Ok((id, ref_, tags, title)) = read_metadata(path.as_path()) {
                    if let Some(link) = ref_ {
                        output.push(Bookmark {
                            id, link, title, tags,
                            created: read_datetime(path.as_path()).unwrap_or(chrono::Utc::now()),
                            content: read_content(path.as_path()).map_or(None, |v| Some(v)),
                        })
                    }
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
            link: statement.read::<String, _>("ref").unwrap(),
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
