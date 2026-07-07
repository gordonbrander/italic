//! Semantic equality between a locally-built record and a record value read
//! back from the PDS, so publish can skip unchanged records and status can
//! detect drift.
//!
//! Both sides are normalized through `serde_json::to_value` — the local record
//! structs, atrium's `BlobRef`, and the `Unknown` values `listRecords` returns
//! all serialize to the same JSON shapes (blob links as `{"$link": …}` maps).
//! The one field that can't be reproduced offline is a blob's `mimeType`:
//! atrium uploads with `Content-Type: */*`, so whatever the PDS recorded is
//! not derivable from the file. Blob identity is `ref.$link` + `size`, both
//! pure functions of the bytes, so comparison strips `mimeType` from blob
//! nodes on both sides.

use serde::Serialize;
use serde_json::Value;

/// True when the two records are semantically identical. A serialization
/// failure on either side compares unequal — the safe direction: publish
/// re-puts, status reports CHANGED.
pub fn equal(local: &impl Serialize, remote: &impl Serialize) -> bool {
    match (serde_json::to_value(local), serde_json::to_value(remote)) {
        (Ok(mut a), Ok(mut b)) => {
            canonicalize(&mut a);
            canonicalize(&mut b);
            a == b
        }
        _ => false,
    }
}

/// Recursively strip `mimeType` from any JSON object carrying
/// `"$type": "blob"` (see module docs for why it can't be compared).
fn canonicalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.get("$type").and_then(Value::as_str) == Some("blob") {
                map.remove("mimeType");
            }
            for v in map.values_mut() {
                canonicalize(v);
            }
        }
        Value::Array(items) => {
            for v in items {
                canonicalize(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn blob_mimetype_difference_is_ignored() {
        let local = json!({
            "$type": "site.standard.document",
            "title": "Hi",
            "coverImage": {
                "$type": "blob",
                "ref": {"$link": "bafkabc"},
                "mimeType": "*/*",
                "size": 11
            }
        });
        let remote = json!({
            "$type": "site.standard.document",
            "title": "Hi",
            "coverImage": {
                "$type": "blob",
                "ref": {"$link": "bafkabc"},
                "mimeType": "image/png",
                "size": 11
            }
        });
        assert!(equal(&local, &remote));
    }

    #[test]
    fn blob_link_and_size_differences_are_detected() {
        let a = json!({"coverImage": {"$type": "blob", "ref": {"$link": "bafkabc"}, "size": 11}});
        let b = json!({"coverImage": {"$type": "blob", "ref": {"$link": "bafkxyz"}, "size": 11}});
        let c = json!({"coverImage": {"$type": "blob", "ref": {"$link": "bafkabc"}, "size": 12}});
        assert!(!equal(&a, &b));
        assert!(!equal(&a, &c));
        assert!(equal(&a, &a));
    }

    #[test]
    fn mimetype_outside_blob_nodes_is_compared() {
        let a = json!({"mimeType": "text/html"});
        let b = json!({"mimeType": "text/plain"});
        assert!(!equal(&a, &b));
    }

    #[test]
    fn absent_vs_present_field_is_unequal() {
        let without = json!({"$type": "site.standard.document", "title": "Hi"});
        let with = json!({
            "$type": "site.standard.document",
            "title": "Hi",
            "coverImage": {"$type": "blob", "ref": {"$link": "bafkabc"}, "size": 11}
        });
        assert!(!equal(&without, &with));
    }
}
