use super::{ScanResult, Severity, Vulnerability};
use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;

pub struct OsvScanner {
    binary_path: PathBuf,
}

impl OsvScanner {
    pub fn new() -> Result<Self> {
        // For now, assume osv-scanner is available. In a real implementation,
        // we'd check if the binary exists
        Ok(Self {
            binary_path: PathBuf::from("osv-scanner"),
        })
    }

    pub async fn scan(&self, path: &PathBuf) -> Result<ScanResult> {
        let output = Command::new(&self.binary_path)
            .args(&[
                "--format=json",
                &path.display().to_string()
            ])
            .output()
            .await?;

        // OSV scanner returns non-zero exit code when vulnerabilities are found
        // So we check stderr for actual errors
        if !output.stderr.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("error") || stderr.contains("Error") {
                return Ok(ScanResult::Error("OSV scan failed".to_string()));
            }
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut vulnerabilities = Vec::new();

        if let Some(results) = response.get("results") {
            for result in results.as_array().unwrap_or(&vec![]) {
                if let Some(packages) = result.get("packages") {
                    for package in packages.as_array().unwrap_or(&vec![]) {
                        if let Some(vulnerabilities_list) = package.get("vulnerabilities") {
                            for vuln_data in vulnerabilities_list.as_array().unwrap_or(&vec![]) {
                                let vuln = Vulnerability {
                                    id: vuln_data.get("id").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                                    title: vuln_data.get("summary").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                                    description: vuln_data.get("details").map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                                    severity: map_severity(vuln_data.get("severity").and_then(|v| v.as_array()).and_then(|arr| arr.first()).and_then(|s| s.get("score")).map(|v| v.as_str().unwrap_or_default()).unwrap_or("")),
                                    category: "dependency".to_string(),
                                    file_path: package.get("package").and_then(|p| p.get("name")).map(|v| v.as_str().unwrap_or_default().to_string()).unwrap_or_default(),
                                    line_number: None,
                                    column_number: None,
                                    code_snippet: None,
                                    suggested_fix: Some(format!("Update {} to a secure version", 
                                        package.get("package").and_then(|p| p.get("name")).map(|v| v.as_str().unwrap_or_default()).unwrap_or("package"))),
                                    references: vuln_data.get("references")
                                        .and_then(|refs| refs.as_array())
                                        .map(|refs| refs.iter()
                                            .filter_map(|r| r.get("url").and_then(|u| u.as_str()).map(|s| s.to_string()))
                                            .collect())
                                        .unwrap_or_default(),
                                    scanner: "osv".to_string(),
                                };
                                vulnerabilities.push(vuln);
                            }
                        }
                    }
                }
            }
        }

        Ok(ScanResult::Success(vulnerabilities))
    }
}

fn map_severity(severity_str: &str) -> Severity {
    // OSV uses CVSS scores, convert to our severity levels
    if let Ok(score) = severity_str.parse::<f32>() {
        match score {
            9.0..=10.0 => Severity::Critical,
            7.0..=8.9 => Severity::High,
            4.0..=6.9 => Severity::Medium,
            0.1..=3.9 => Severity::Low,
            _ => Severity::Info,
        }
    } else {
        // Fallback to string matching
        match severity_str.to_uppercase().as_str() {
            "CRITICAL" => Severity::Critical,
            "HIGH" => Severity::High,
            "MODERATE" | "MEDIUM" => Severity::Medium,
            "LOW" => Severity::Low,
            _ => Severity::Info,
        }
    }
}
