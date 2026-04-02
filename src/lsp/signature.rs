//! Signature help (function parameter hints).

use serde_json::Value as JsonValue;

/// Signature help result.
#[derive(Debug, Clone)]
pub struct SignatureHelpResult {
    /// Available signatures.
    pub signatures: Vec<SignatureInfo>,
    /// Index of the active signature.
    pub active_signature: usize,
    /// Index of the active parameter.
    pub active_parameter: usize,
}

/// A function signature.
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    /// Full signature label (e.g., "fn foo(x: i32, y: &str) -> bool").
    pub label: String,
    /// Documentation (if any).
    pub documentation: Option<String>,
    /// Parameter info.
    pub parameters: Vec<ParameterInfo>,
}

/// A parameter within a signature.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// Parameter label (name or range within signature).
    pub label: String,
    /// Start offset in the signature label.
    pub label_start: Option<usize>,
    /// End offset in the signature label.
    pub label_end: Option<usize>,
    /// Documentation.
    pub documentation: Option<String>,
}

/// Parses a signature help response.
pub fn parse_signature_help(result: JsonValue) -> Option<SignatureHelpResult> {
    if result.is_null() {
        return None;
    }

    let signatures_val = result.get("signatures")?.as_array()?;
    if signatures_val.is_empty() {
        return None;
    }

    let signatures: Vec<SignatureInfo> = signatures_val
        .iter()
        .filter_map(|sig| {
            let label = sig.get("label")?.as_str()?.to_string();
            let documentation = sig.get("documentation").and_then(|d| {
                d.as_str().map(|s| s.to_string()).or_else(|| {
                    d.get("value")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
            });

            let parameters = sig
                .get("parameters")
                .and_then(|p| p.as_array())
                .map(|params| {
                    params
                        .iter()
                        .filter_map(|param| {
                            let (param_label, label_start, label_end) =
                                parse_parameter_label(param)?;
                            let doc = param.get("documentation").and_then(|d| {
                                d.as_str().map(|s| s.to_string()).or_else(|| {
                                    d.get("value")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                })
                            });
                            Some(ParameterInfo {
                                label: param_label,
                                label_start,
                                label_end,
                                documentation: doc,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            Some(SignatureInfo {
                label,
                documentation,
                parameters,
            })
        })
        .collect();

    let active_signature = result
        .get("activeSignature")
        .and_then(|a| a.as_u64())
        .unwrap_or(0) as usize;
    let active_parameter = result
        .get("activeParameter")
        .and_then(|a| a.as_u64())
        .unwrap_or(0) as usize;

    Some(SignatureHelpResult {
        signatures,
        active_signature,
        active_parameter,
    })
}

fn parse_parameter_label(param: &JsonValue) -> Option<(String, Option<usize>, Option<usize>)> {
    let label_val = param.get("label")?;

    if let Some(s) = label_val.as_str() {
        Some((s.to_string(), None, None))
    } else if let Some(arr) = label_val.as_array() {
        // [start, end] offsets into the signature label
        let start = arr.first()?.as_u64()? as usize;
        let end = arr.get(1)?.as_u64()? as usize;
        Some((String::new(), Some(start), Some(end)))
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_signature_help() {
        let resp = json!({
            "signatures": [{
                "label": "fn foo(x: i32, y: &str) -> bool",
                "parameters": [
                    {"label": "x: i32"},
                    {"label": "y: &str"}
                ]
            }],
            "activeSignature": 0,
            "activeParameter": 1
        });
        let result = parse_signature_help(resp).unwrap();
        assert_eq!(result.signatures.len(), 1);
        assert_eq!(result.active_parameter, 1);
        assert_eq!(result.signatures[0].parameters.len(), 2);
        assert_eq!(result.signatures[0].parameters[0].label, "x: i32");
    }

    #[test]
    fn test_parse_signature_help_empty() {
        let resp = json!({"signatures": []});
        assert!(parse_signature_help(resp).is_none());
    }

    #[test]
    fn test_parse_signature_help_null() {
        assert!(parse_signature_help(JsonValue::Null).is_none());
    }

    #[test]
    fn test_parse_signature_with_offset_labels() {
        let resp = json!({
            "signatures": [{
                "label": "fn bar(a: u8, b: String)",
                "parameters": [
                    {"label": [7, 12]},
                    {"label": [14, 23]}
                ]
            }],
            "activeSignature": 0,
            "activeParameter": 0
        });
        let result = parse_signature_help(resp).unwrap();
        assert!(result.signatures[0].parameters[0].label_start.is_some());
        assert_eq!(result.signatures[0].parameters[0].label_start, Some(7));
    }

    #[test]
    fn test_parse_signature_with_documentation() {
        let resp = json!({
            "signatures": [{
                "label": "fn test()",
                "documentation": "A test function",
                "parameters": []
            }],
            "activeSignature": 0,
            "activeParameter": 0
        });
        let result = parse_signature_help(resp).unwrap();
        assert_eq!(
            result.signatures[0].documentation.as_deref(),
            Some("A test function")
        );
    }

    #[test]
    fn test_parse_signature_with_markup_documentation() {
        let resp = json!({
            "signatures": [{
                "label": "fn test()",
                "documentation": {"kind": "markdown", "value": "**bold**"},
                "parameters": []
            }],
            "activeSignature": 0,
            "activeParameter": 0
        });
        let result = parse_signature_help(resp).unwrap();
        assert_eq!(
            result.signatures[0].documentation.as_deref(),
            Some("**bold**")
        );
    }
}
