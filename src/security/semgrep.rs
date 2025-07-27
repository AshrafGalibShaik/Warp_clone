use super::{ScanResult, Severity, Vulnerability};
use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;

pub struct SemgrepScanner {
    binary_path: PathBuf,
}

impl SemgrepScanner {
    pub fn new() -> Result<Self> {
        // For now, assume semgrep is available. In a real implementation,
        // we'd check if the binary exists
        Ok(Self {
            binary_path: PathBuf::from("semgrep"),
        })
    }

    pub async fn scan(&self, path: &PathBuf) -> Result<ScanResult> {
        let output = Command::new(&self.binary_path)
            .args(&[
                "--config=auto", 
                "--json", 
                &path.display().to_string()
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(ScanResult::Error("Semgrep scan failed".to_string()));
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut vulnerabilities = Vec::new();

        if let Some(results) = response.get("results") {
            for result in results.as_array().unwrap_or(&vec![]) {
                let vuln = Vulnerability {
                    id: result.get("check_id").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    title: result.get("message").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    description: result.get("message").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    severity: map_severity(result.get("severity").map(|v| v.as_str().unwrap_or_default()).unwrap_or("")),
                    category: "code-quality".to_string(),
                    file_path: result.get("path").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    line_number: result.get("start").and_then(|v| v.get("line")).and_then(|v| v.as_u64()).map(|v| v as usize),
                    column_number: result.get("start").and_then(|v| v.get("col")).and_then(|v| v.as_u64()).map(|v| v as usize),
                    code_snippet: result.get("extra").and_then(|v| v.get("lines")).map(|v| v.as_str().unwrap_or_default().to_string()),
                    suggested_fix: None,
                    references: vec![],
                    scanner: "semgrep".to_string(),
                };
                vulnerabilities.push(vuln);
            }
        }

        Ok(ScanResult::Success(vulnerabilities))
    }

    pub async fn quick_scan(&self, path: &PathBuf) -> Result<ScanResult> {
        let output = Command::new(&self.binary_path)
            .args(&[
                "--config=p/security-audit",
                "--json",
                "--severity=HIGH",
                &path.display().to_string()
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(ScanResult::Error("Semgrep quick scan failed".to_string()));
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut vulnerabilities = Vec::new();

        if let Some(results) = response.get("results") {
            for result in results.as_array().unwrap_or(&vec![]) {
                let vuln = Vulnerability {
                    id: result.get("check_id").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    title: result.get("message").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    description: result.get("message").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    severity: map_severity(result.get("severity").map(|v| v.as_str().unwrap_or_default()).unwrap_or("")),
                    category: "security".to_string(),
                    file_path: result.get("path").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                    line_number: result.get("start").and_then(|v| v.get("line")).and_then(|v| v.as_u64()).map(|v| v as usize),
                    column_number: result.get("start").and_then(|v| v.get("col")).and_then(|v| v.as_u64()).map(|v| v as usize),
                    code_snippet: result.get("extra").and_then(|v| v.get("lines")).map(|v| v.as_str().unwrap_or_default().to_string()),
                    suggested_fix: None,
                    references: vec![],
                    scanner: "semgrep".to_string(),
                };
                vulnerabilities.push(vuln);
            }
        }

        Ok(ScanResult::Success(vulnerabilities))
    }
}

fn map_severity(severity: &str) -> Severity {
    match severity.to_uppercase().as_str() {
        "ERROR" | "HIGH" => Severity::High,
        "WARNING" | "MEDIUM" => Severity::Medium,
        "INFO" | "LOW" => Severity::Low,
        _ => Severity::Info,
    }
}
