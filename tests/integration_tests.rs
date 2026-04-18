use std::fs;
use std::path::PathBuf;

fn get_fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(name);
    path
}

#[test]
fn test_fixtures_exist() {
    let fixtures = vec![
        "sample_deals.csv",
        "sample_report.htm",
        "sample_report.htm.xml",
    ];

    for fixture in fixtures {
        let path = get_fixture_path(fixture);
        assert!(path.exists(), "Fixture {} should exist", fixture);
    }
}

#[test]
fn test_sample_deals_csv_format() {
    let path = get_fixture_path("sample_deals.csv");
    let content = fs::read_to_string(path).expect("Should read sample_deals.csv");

    // Check CSV has header and data rows
    let lines: Vec<&str> = content.lines().collect();
    assert!(!lines.is_empty(), "CSV should have at least a header");

    // Check for expected columns in header
    let header = lines[0];
    assert!(header.contains("Time") || header.contains("time"), "Header should contain Time column");
}

#[test]
fn test_sample_report_html_format() {
    let path = get_fixture_path("sample_report.htm");
    let content = fs::read_to_string(path).expect("Should read sample_report.htm");

    // Check HTML structure
    assert!(content.contains("<html") || content.contains("<table"),
            "Report should contain HTML or table elements");
}

#[test]
fn test_sample_report_xml_format() {
    let path = get_fixture_path("sample_report.htm.xml");
    let content = fs::read_to_string(path).expect("Should read sample_report.htm.xml");

    // Check XML structure
    assert!(content.contains("<?xml") || content.contains("<Workbook"),
            "Report should contain XML or Workbook elements");
}
