use crate::runner::SuiteOutcome;

/// Minimal JUnit XML for CI systems (escape-only, no timing granularity).
pub fn render_junit(suite_name: &str, outcome: &SuiteOutcome) -> String {
    let tests = outcome.scenarios.len();
    let failures = outcome
        .scenarios
        .iter()
        .filter(|s| !s.passed && !s.skipped)
        .count();
    let skipped = outcome.scenarios.iter().filter(|s| s.skipped).count();
    let suite_name_esc = xml_escape(suite_name);
    let mut out = String::new();
    out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    out.push('\n');
    out.push_str(&format!(
        r#"<testsuites name="{suite_name_esc}" tests="{tests}" failures="{failures}" skipped="{skipped}" errors="0">"#
    ));
    out.push('\n');
    out.push_str(&format!(
        r#"  <testsuite name="{suite_name_esc}" tests="{tests}" failures="{failures}" skipped="{skipped}" errors="0">"#
    ));
    out.push('\n');

    for scenario in &outcome.scenarios {
        let name_esc = xml_escape(&scenario.name);
        out.push_str(&format!(
            r#"    <testcase classname="mcp_check" name="{name_esc}">"#
        ));
        out.push('\n');
        if scenario.skipped {
            out.push_str(r#"      <skipped message="filtered by capabilities"/>"#);
            out.push('\n');
        } else if !scenario.passed {
            if let Some(msg) = &scenario.error {
                let body_esc = xml_escape(msg);
                out.push_str(&format!(
                    r#"      <failure message="scenario failed">{body_esc}</failure>"#
                ));
                out.push('\n');
            } else {
                out.push_str(r#"      <failure message="scenario failed"/>"#);
                out.push('\n');
            }
        }
        out.push_str("    </testcase>\n");
    }

    out.push_str("  </testsuite>\n");
    out.push_str("</testsuites>\n");
    out
}

fn xml_escape(input: &str) -> String {
    let mut buf = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => buf.push_str("&amp;"),
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '"' => buf.push_str("&quot;"),
            '\'' => buf.push_str("&apos;"),
            _ if ch.is_control() => {}
            _ => buf.push(ch),
        }
    }
    buf
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::runner::{ScenarioOutcome, SuiteOutcome};

    #[test]
    fn render_escapes_failure_payload() {
        let outcome = SuiteOutcome {
            passed: false,
            scenarios: vec![ScenarioOutcome {
                name: "bad&name".to_string(),
                passed: false,
                skipped: false,
                error: Some("<oops>".to_string()),
            }],
        };
        let xml = render_junit("suite&", &outcome);
        assert!(xml.contains("bad&amp;name"));
        assert!(xml.contains("&lt;oops&gt;"));
        assert!(xml.contains("suite&amp;"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml_specials() {
        assert_eq!(xml_escape("a&b"), "a&amp;b");
        assert_eq!(xml_escape("<t>"), "&lt;t&gt;");
    }
}
