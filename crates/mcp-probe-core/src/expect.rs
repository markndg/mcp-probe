use serde_json::Value;
use std::fmt;
use thiserror::Error;

/// Explains why a structured expectation did not match an actual JSON value.
#[derive(Debug, Error)]
pub struct MatchFailure {
    path: String,
    detail: String,
}

impl fmt::Display for MatchFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} — {}", self.path, self.detail)
    }
}

impl MatchFailure {
    fn new(path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            detail: detail.into(),
        }
    }
}

/// Deep equality for JSON values (strict matching).
pub fn strict_equal(expected: &Value, actual: &Value) -> Result<(), MatchFailure> {
    if expected == actual {
        Ok(())
    } else {
        Err(MatchFailure::new(
            "$",
            format!("expected {expected}, got {actual}"),
        ))
    }
}

/// Returns `Ok(())` when `actual` contains `expected` using object subset rules.
///
/// - Objects: every key in `expected` must exist in `actual` with recursively matching values.
/// - Arrays: each element of `expected` must match at least one unused element of `actual`
///   using the same subset rules (order-independent), unless `ordered_arrays` is true.
/// - Scalars: must be equal (`==`).
pub fn subset_match(expected: &Value, actual: &Value) -> Result<(), MatchFailure> {
    subset_match_with_options(expected, actual, false)
}

/// Same as [`subset_match`], but array comparisons can be forced to index-aligned subset matching.
pub fn subset_match_with_options(
    expected: &Value,
    actual: &Value,
    ordered_arrays: bool,
) -> Result<(), MatchFailure> {
    subset_match_at("$", expected, actual, ordered_arrays)
}

fn subset_match_at(
    path: &str,
    expected: &Value,
    actual: &Value,
    ordered_arrays: bool,
) -> Result<(), MatchFailure> {
    match (expected, actual) {
        (Value::Object(exp_obj), Value::Object(act_obj)) => {
            for (key, exp_val) in exp_obj {
                let child_path = format!("{path}.{key}");
                let act_val = act_obj.get(key).ok_or_else(|| {
                    MatchFailure::new(
                        child_path.clone(),
                        format!("missing key `{key}` in actual object"),
                    )
                })?;
                subset_match_at(&child_path, exp_val, act_val, ordered_arrays)?;
            }
            Ok(())
        }
        (Value::Array(exp_arr), Value::Array(act_arr)) if ordered_arrays => {
            if exp_arr.len() != act_arr.len() {
                return Err(MatchFailure::new(
                    path.to_string(),
                    format!(
                        "ordered array length mismatch: expected {} elements, got {}",
                        exp_arr.len(),
                        act_arr.len()
                    ),
                ));
            }
            for (idx, (exp_item, act_item)) in exp_arr.iter().zip(act_arr.iter()).enumerate() {
                let child_path = format!("{path}[{idx}]");
                subset_match_at(&child_path, exp_item, act_item, ordered_arrays)?;
            }
            Ok(())
        }
        (Value::Array(exp_arr), Value::Array(act_arr)) => {
            let mut used = vec![false; act_arr.len()];
            for (idx, exp_item) in exp_arr.iter().enumerate() {
                let child_path = format!("{path}[{idx}]");
                let mut matched = false;
                for (j, act_item) in act_arr.iter().enumerate() {
                    if used[j] {
                        continue;
                    }
                    if subset_match_at(&child_path, exp_item, act_item, ordered_arrays).is_ok() {
                        used[j] = true;
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return Err(MatchFailure::new(
                        child_path,
                        "no matching element found in actual array",
                    ));
                }
            }
            Ok(())
        }
        _ if expected == actual => Ok(()),
        _ => Err(MatchFailure::new(
            path.to_string(),
            format!("expected {expected}, got {actual}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_subset_ok() {
        let exp = json!({"a": 1, "b": {"c": 2}});
        let act = json!({"a": 1, "b": {"c": 2, "d": 3}, "x": 9});
        subset_match(&exp, &act).unwrap();
    }

    #[test]
    fn object_subset_missing_key() {
        let exp = json!({"a": 1});
        let act = json!({"b": 1});
        assert!(subset_match(&exp, &act).is_err());
    }

    #[test]
    fn array_order_independent() {
        let exp = json!([{"name": "b"}, {"name": "a"}]);
        let act = json!([{"name": "a", "id": 1}, {"name": "b", "id": 2}]);
        subset_match(&exp, &act).unwrap();
    }

    #[test]
    fn ordered_arrays_require_alignment() {
        let exp = json!([{"k": 1}, {"k": 2}]);
        let act = json!([{"k": 2}, {"k": 1}]);
        assert!(subset_match_with_options(&exp, &act, true).is_err());
        subset_match_with_options(&exp, &act, false).unwrap();
    }

    #[test]
    fn strict_equal_checks_exact() {
        let exp = json!({"a": 1});
        let act = json!({"a": 1, "b": 2});
        assert!(strict_equal(&exp, &act).is_err());
    }
}
