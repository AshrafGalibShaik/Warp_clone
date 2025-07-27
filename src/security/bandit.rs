use super::{ScanResult, Severity, Vulnerability};
use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;

pub struct BanditScanner {
    binary_path: PathBuf,
}

impl BanditScanner {
    pub fn new() -> Result<Self> {
        // Try to find bandit in common locations
        let _possible_paths: Vec<PathBuf> = vec![];

        // For now, assume bandit is available. In a real implementation,
        // we'd check if the binary exists
        Ok(Self {
            binary_path: PathBuf::from("bandit"),
        })
    }

    pub async fn scan(&self, path: &PathBuf) -> Result<ScanResult> {
        let output = Command::new(&self.binary_path)
            .args(&["-r", &path.display().to_string(), "-f", "json"])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(ScanResult::Error("Bandit scan failed".to_string()));
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut vulnerabilities = Vec::new();

        if let Some(results) = response.get("results") {
            for result in results.as_array().unwrap_or(&vec![]) {
                let vuln = Vulnerability {
                    id: result
                        .get("test_id")
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .unwrap_or_default(),
                    title: result
                        .get("issue_text")
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .unwrap_or_default(),
                    description: result
                        .get("issue_confidence")
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .unwrap_or_default(),
                    severity: map_severity(
                        result
                            .get("issue_severity")
                            .map(|v| v.as_str().unwrap_or_default())
                            .unwrap_or(""),
                    ),
                    category: "python-code".to_string(),
                    file_path: result
                        .get("filename")
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .unwrap_or_default(),
                    line_number: result
                        .get("line_number")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize),
                    column_number: None,
                    code_snippet: None,
                    suggested_fix: None,
                    references: vec![],
                    scanner: "bandit".to_string(),
                };
                vulnerabilities.push(vuln);
            }
        }

        Ok(ScanResult::Success(vulnerabilities))
    }
}

fn map_severity(severity: &str) -> Severity {
    match severity.to_lowercase().as_str() {
        "high" => Severity::High,
        "medium" => Severity::Medium,
        "low" => Severity::Low,
        _ => Severity::Info,
    }
}
