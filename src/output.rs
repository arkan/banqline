use std::collections::HashMap;
use std::io::{self, Write};

use anyhow::{Context, Result};
use serde::Serialize;
use tabled::{builder::Builder, settings::Style};

/// Returns a human-friendly account label.
///
/// With alias: `"principal (FR76...65133)"`. Without: full IBAN.
pub fn account_display_name(alias: &str, iban: &str) -> String {
    if alias.is_empty() {
        return iban.to_string();
    }
    let suffix = if iban.len() > 9 {
        format!("{}...{}", &iban[..4], &iban[iban.len() - 5..])
    } else {
        iban.to_string()
    };
    format!("{alias} ({suffix})")
}

/// Returns the last `n` characters of an IBAN, prefixed with `"..."`.
pub fn iban_suffix(iban: &str, n: usize) -> String {
    if iban.len() <= n {
        return iban.to_string();
    }
    format!("...{}", &iban[iban.len() - n..])
}

/// Printer formats output as table (borderless), JSON, or CSV.
pub struct Printer {
    pub json: bool,
    pub csv: bool,
}

impl Printer {
    /// Writes headers and rows as a borderless aligned table to stdout.
    pub fn print_table(&self, headers: Vec<String>, rows: Vec<Vec<String>>) -> Result<()> {
        if self.csv {
            return self.print_csv_to(&mut io::stdout(), headers, rows);
        }
        self.print_table_to(&mut io::stdout(), headers, rows)
    }

    fn print_table_to(
        &self,
        w: &mut dyn Write,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    ) -> Result<()> {
        let mut builder = Builder::default();
        builder.push_record(headers);
        for row in rows {
            builder.push_record(row);
        }
        let mut table = builder.build();
        table.with(Style::blank());
        write!(w, "{table}").context("write table")
    }

    fn print_csv_to(
        &self,
        w: &mut dyn Write,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    ) -> Result<()> {
        write_csv_record(w, &headers)?;
        for row in rows {
            write_csv_record(w, &row)?;
        }
        Ok(())
    }

    /// Outputs any serializable value as indented JSON to stdout.
    pub fn print_json<T: Serialize>(&self, v: &T) -> Result<()> {
        self.print_json_to(&mut io::stdout(), v)
    }

    fn print_json_to<T: Serialize>(&self, w: &mut dyn Write, v: &T) -> Result<()> {
        let data = serde_json::to_string_pretty(v).context("marshal json")?;
        writeln!(w, "{data}").context("write json")
    }

    /// Writes a table followed by a footer line (e.g. "Last synced: ...").
    pub fn print_table_with_footer(
        &self,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        footer: &str,
    ) -> Result<()> {
        self.print_table(headers, rows)?;
        if !footer.is_empty() {
            println!("\n{footer}");
        }
        Ok(())
    }

    /// Outputs a JSON envelope with `data` and additional metadata fields.
    pub fn print_json_with_meta<T: Serialize>(
        &self,
        v: &T,
        meta: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let data = serde_json::to_value(v).context("convert to json value")?;
        let mut envelope = serde_json::Map::new();
        envelope.insert("data".into(), data);
        for (k, val) in meta {
            envelope.insert(k, val);
        }
        self.print_json(&envelope)
    }
}

fn write_csv_record(w: &mut dyn Write, fields: &[String]) -> Result<()> {
    for (idx, field) in fields.iter().enumerate() {
        if idx > 0 {
            write!(w, ",").context("write csv separator")?;
        }
        write!(w, "{}", escape_csv_field(field)).context("write csv field")?;
    }
    writeln!(w).context("write csv newline")
}

fn escape_csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_display_name_with_alias() {
        let got = account_display_name("principal", "FR7612345678901234565133");
        assert_eq!(got, "principal (FR76...65133)");
    }

    #[test]
    fn test_account_display_name_without_alias() {
        let got = account_display_name("", "FR7612345678901234565133");
        assert_eq!(got, "FR7612345678901234565133");
    }

    #[test]
    fn test_account_display_name_short_iban() {
        let got = account_display_name("test", "FR76");
        assert_eq!(got, "test (FR76)");
    }

    #[test]
    fn test_print_table() {
        let mut buf = Vec::new();
        let p = Printer {
            json: false,
            csv: false,
        };

        let headers = vec!["NAME".into(), "IBAN".into(), "CURRENCY".into()];
        let rows = vec![
            vec!["Main".into(), "DE89370400440532013000".into(), "EUR".into()],
            vec![
                "Savings".into(),
                "FR7630006000011234567890189".into(),
                "EUR".into(),
            ],
        ];

        p.print_table_to(&mut buf, headers, rows).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("NAME"));
        assert!(out.contains("IBAN"));
        assert!(out.contains("CURRENCY"));
        assert!(out.contains("Main"));
        assert!(out.contains("DE89370400440532013000"));
        assert!(out.contains("Savings"));
    }

    #[test]
    fn test_print_table_empty() {
        let mut buf = Vec::new();
        let p = Printer {
            json: false,
            csv: false,
        };

        p.print_table_to(&mut buf, vec!["COL".into()], vec![])
            .unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("COL"));
    }

    #[test]
    fn test_print_json() {
        let mut buf = Vec::new();
        let p = Printer {
            json: true,
            csv: false,
        };

        let mut input = serde_json::Map::new();
        input.insert("name".into(), "test".into());
        input.insert("value".into(), "42".into());

        p.print_json_to(&mut buf, &input).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["value"], "42");
    }

    #[test]
    fn test_print_json_indented() {
        let mut buf = Vec::new();
        let p = Printer {
            json: true,
            csv: false,
        };

        let mut input = serde_json::Map::new();
        input.insert("a".into(), serde_json::Value::Number(1.into()));

        p.print_json_to(&mut buf, &input).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("  \"a\": 1"));
    }
}
