//! Parsing `docker ... --format {{json .}}` output.
//!
//! Only the CLI transport needs this. [`super::client`] gets typed structs
//! from the daemon and does not come through here.

use super::container::{DockerContainer, DockerImage};
use super::create::DockerSearchResult;

/// Maximum number of items to parse from Docker output.
const MAX_PARSE_ITEMS: usize = 500;

/// Upper bound on one field's length, so a malformed line cannot make the
/// parser walk the rest of the buffer.
const MAX_FIELD_BYTES: usize = 1000;

/// Extracts a field value from a JSON object string.
///
/// Handles the flat `{"field":"value",...}` shape Docker's Go templates emit.
/// Returns `None` when the field is absent.
#[must_use]
pub(super) fn extract_json_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", field);
    let start = json.find(&pattern)?;
    let value_start = start + pattern.len();

    let rest = &json[value_start..];
    let mut value_end = 0;
    let mut in_escape = false;

    for c in rest.chars() {
        if in_escape {
            in_escape = false;
            value_end += c.len_utf8();
        } else if c == '\\' {
            in_escape = true;
            value_end += 1;
        } else if c == '"' {
            break;
        } else {
            value_end += c.len_utf8();
        }

        if value_end > MAX_FIELD_BYTES {
            break;
        }
    }

    let value = &rest[..value_end];

    let unescaped = value
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
        .replace("\\/", "/")
        .replace("\\n", "\n")
        .replace("\\t", "\t");

    Some(unescaped)
}

/// Parses a single JSON line from `docker ps` output.
#[must_use]
pub(super) fn parse_container_line(json_line: &str) -> Option<DockerContainer> {
    let id = extract_json_field(json_line, "ID")?;
    if id.is_empty() {
        return None;
    }
    let names = extract_json_field(json_line, "Names").unwrap_or_default();
    let image = extract_json_field(json_line, "Image").unwrap_or_default();
    let status = extract_json_field(json_line, "Status").unwrap_or_default();
    let ports = extract_json_field(json_line, "Ports").unwrap_or_default();
    let created = extract_json_field(json_line, "CreatedAt").unwrap_or_default();

    let name = names.trim_start_matches('/').to_string();

    let mut container = DockerContainer::new(id, name, image, status);
    container.created = created;

    if !ports.is_empty() {
        container.ports = ports
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
    }

    Some(container)
}

/// Parses JSON output from `docker ps --format {{json .}}`.
#[must_use]
pub(super) fn parse_containers_json(output: &str, running_only: bool) -> Vec<DockerContainer> {
    let mut containers = Vec::new();

    for line in output.lines() {
        if containers.len() >= MAX_PARSE_ITEMS {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(container) = parse_container_line(line)
            && (!running_only || container.status.is_running())
        {
            containers.push(container);
        }
    }

    containers
}

/// Parses a single JSON line from `docker images` output.
#[must_use]
pub(super) fn parse_image_line(json_line: &str) -> Option<DockerImage> {
    let id = extract_json_field(json_line, "ID")?;
    if id.is_empty() {
        return None;
    }
    let repository = extract_json_field(json_line, "Repository").unwrap_or_default();
    let tag = extract_json_field(json_line, "Tag").unwrap_or_default();
    let size = extract_json_field(json_line, "Size").unwrap_or_default();
    let created = extract_json_field(json_line, "CreatedAt").unwrap_or_default();

    let mut image = DockerImage::new(id, repository, tag);
    image.size = size;
    image.created = created;

    Some(image)
}

/// Parses JSON output from `docker images --format {{json .}}`.
///
/// Intermediate layers report `<none>` as their repository; they are dropped
/// because they cannot be run and only pad the list.
#[must_use]
pub(super) fn parse_images_json(output: &str) -> Vec<DockerImage> {
    let mut images = Vec::new();

    for line in output.lines() {
        if images.len() >= MAX_PARSE_ITEMS {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(image) = parse_image_line(line)
            && image.repository != "<none>"
        {
            images.push(image);
        }
    }

    images
}

/// Parses a single Docker Hub search result line.
#[must_use]
pub(super) fn parse_search_line(json_line: &str) -> Option<DockerSearchResult> {
    let name = extract_json_field(json_line, "Name")?;
    if name.is_empty() {
        return None;
    }

    let description = extract_json_field(json_line, "Description").unwrap_or_default();
    let stars = extract_json_field(json_line, "StarCount")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let official = extract_json_field(json_line, "IsOfficial")
        .map(|s| s == "[OK]" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    Some(DockerSearchResult {
        name,
        description,
        stars,
        official,
    })
}

/// Parses Docker Hub search results from JSON output.
#[must_use]
pub(super) fn parse_search_results(output: &str) -> Vec<DockerSearchResult> {
    output
        .lines()
        .take(MAX_PARSE_ITEMS)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(parse_search_line)
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_field() {
        let json = r#"{"ID":"abc123","Names":"/my-container","Image":"nginx:latest"}"#;

        assert_eq!(extract_json_field(json, "ID"), Some("abc123".to_string()));
        assert_eq!(
            extract_json_field(json, "Names"),
            Some("/my-container".to_string())
        );
        assert_eq!(
            extract_json_field(json, "Image"),
            Some("nginx:latest".to_string())
        );
        assert_eq!(extract_json_field(json, "Missing"), None);
    }

    #[test]
    fn an_escaped_quote_does_not_end_the_value() {
        let json = r#"{"Command":"sh -c \"echo hi\"","ID":"x"}"#;
        assert_eq!(
            extract_json_field(json, "Command"),
            Some("sh -c \"echo hi\"".to_string())
        );
        assert_eq!(extract_json_field(json, "ID"), Some("x".to_string()));
    }

    #[test]
    fn a_multibyte_value_is_not_split_mid_character() {
        let json = r#"{"Names":"café-service","ID":"x"}"#;
        assert_eq!(
            extract_json_field(json, "Names"),
            Some("café-service".to_string())
        );
    }

    #[test]
    fn test_parse_container_line() {
        let json = r#"{"ID":"abc123def456","Names":"/my-nginx","Image":"nginx:latest","Status":"Up 5 minutes","Ports":"0.0.0.0:8080->80/tcp","CreatedAt":"2024-01-01 10:00:00"}"#;

        let container = parse_container_line(json).unwrap();

        assert_eq!(container.id, "abc123def456");
        assert_eq!(container.name, "my-nginx");
        assert_eq!(container.image, "nginx:latest");
        assert!(container.is_running());
        assert_eq!(container.ports.len(), 1);
        assert_eq!(container.ports[0], "0.0.0.0:8080->80/tcp");
    }

    #[test]
    fn a_line_without_an_id_is_dropped_rather_than_panicking() {
        // `DockerContainer::new` asserts on an empty id, so the parser must
        // filter these out before constructing one.
        assert!(parse_container_line(r#"{"Names":"orphan"}"#).is_none());
        assert!(parse_container_line(r#"{"ID":"","Names":"orphan"}"#).is_none());
        assert!(parse_image_line(r#"{"ID":"","Repository":"nginx"}"#).is_none());
        assert!(parse_search_line(r#"{"Name":""}"#).is_none());
    }

    #[test]
    fn blank_lines_and_junk_are_skipped_by_the_list_parsers() {
        let output = "\n  \n{\"ID\":\"a\",\"Status\":\"Up\"}\nnot json\n{\"ID\":\"b\",\"Status\":\"Exited (0)\"}\n";
        let all = parse_containers_json(output, false);
        assert_eq!(all.len(), 2);

        let running = parse_containers_json(output, true);
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, "a");
    }

    #[test]
    fn an_empty_buffer_parses_to_nothing() {
        assert!(parse_containers_json("", false).is_empty());
        assert!(parse_images_json("").is_empty());
        assert!(parse_search_results("").is_empty());
    }

    #[test]
    fn test_parse_image_line() {
        let json = r#"{"ID":"sha256:abc123","Repository":"nginx","Tag":"latest","Size":"150MB","CreatedAt":"2024-01-01"}"#;

        let image = parse_image_line(json).unwrap();

        assert_eq!(image.id, "sha256:abc123");
        assert_eq!(image.repository, "nginx");
        assert_eq!(image.tag, "latest");
        assert_eq!(image.size, "150MB");
        assert_eq!(image.full_name(), "nginx:latest");
    }

    #[test]
    fn dangling_layers_are_left_out_of_the_image_list() {
        let output = "{\"ID\":\"a\",\"Repository\":\"<none>\",\"Tag\":\"<none>\"}\n{\"ID\":\"b\",\"Repository\":\"nginx\",\"Tag\":\"latest\"}";
        let images = parse_images_json(output);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].repository, "nginx");
    }

    #[test]
    fn a_search_result_reads_stars_and_the_official_flag() {
        let ok = r#"{"Name":"nginx","Description":"web","StarCount":"20000","IsOfficial":"[OK]"}"#;
        let parsed = parse_search_line(ok).unwrap();
        assert_eq!(parsed.stars, 20_000);
        assert!(parsed.official);

        let truthy = r#"{"Name":"a","StarCount":"1","IsOfficial":"TRUE"}"#;
        assert!(parse_search_line(truthy).unwrap().official);

        let unofficial = r#"{"Name":"b","StarCount":"not a number"}"#;
        let parsed = parse_search_line(unofficial).unwrap();
        assert_eq!(parsed.stars, 0, "an unparsable star count reads as zero");
        assert!(!parsed.official);
    }

    #[test]
    fn the_parsers_stop_at_the_item_cap() {
        let line = "{\"ID\":\"a\",\"Status\":\"Up\"}\n";
        let output = line.repeat(MAX_PARSE_ITEMS + 50);
        assert_eq!(parse_containers_json(&output, false).len(), MAX_PARSE_ITEMS);
    }

    #[test]
    fn an_unterminated_field_stops_at_the_length_bound() {
        let json = format!("{{\"ID\":\"{}", "x".repeat(MAX_FIELD_BYTES + 200));
        let value = extract_json_field(&json, "ID").unwrap();
        assert!(value.len() <= MAX_FIELD_BYTES + 1, "{}", value.len());
    }
}
