use super::{UiDocument, UiDocumentError, ValidatedUiDocument};
use serde_json::Value;

impl UiDocument {
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        sort_json_objects(&mut value);
        serde_json::to_string(&value)
    }

    pub fn to_canonical_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        sort_json_objects(&mut value);
        serde_json::to_string_pretty(&value).map(|mut output| {
            output.push('\n');
            output
        })
    }

    pub fn parse_and_validate_json(source: &str) -> Result<ValidatedUiDocument, UiDocumentError> {
        ValidatedUiDocument::parse_json(source)
    }
}

fn sort_json_objects(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let mut entries = std::mem::take(object).into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            for (_, child) in &mut entries {
                sort_json_objects(child);
            }
            object.extend(entries);
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json_objects),
        Value::Number(number) if number.is_f64() && number.as_f64() == Some(0.0) => {
            *number = serde_json::Number::from_f64(0.0).expect("zero is finite");
        }
        _ => {}
    }
}
