use serde_json;
use std::collections::HashMap;

/// Mapping of known GitHub usernames to preferred local aliases.
pub fn get_users() -> HashMap<String, String> {
    let users = r#"{
        "afck": "Andreas",
        "dirvine": "David",
        "fizyk20": "Bart",
        "Fraser999": "Fraser",
        "krishnaIndia": "Krishna",
        "madadam": "Adam",
        "maidsafe": "MaidSafe",
        "maqi": "Qi",
        "michaelsproul": "Michael",
        "nbaksalyar": "Nikita",
        "NickLambert": "Nick",
        "shankar2015": "Shankar",
        "ustulation": "Spandan",
        "Viv-Rajkumar": "Viv"
    }"#;
    unwrap!(serde_json::from_str(users))
}
