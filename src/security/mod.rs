pub mod scanner;
pub mod bandit;
pub mod semgrep;
pub mod osv;

pub use scanner::{SecurityScanner, ScanResult, Vulnerability, Severity};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_bandit: bool,
    pub enable_semgrep: bool,
    pub enable_osv: bool,
    pub scan_timeout_seconds: u64,
    pub max_file_size_mb: u64,
    pub excluded_paths: Vec<String>,
    pub bandit_config_path: Option<PathBuf>,
    pub semgrep_rules_path: Option<PathBuf>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_bandit: true,
            enable_semgrep: true,
            enable_osv: true,
            scan_timeout_seconds: 300, // 5 minutes
            max_file_size_mb: 10,
            excluded_paths: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "__pycache__".to_string(),
                ".venv".to_string(),
                "venv".to_string(),
            ],
            bandit_config_path: None,
            semgrep_rules_path: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityScanRequest {
    pub path: PathBuf,
    pub scan_type: ScanType,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ScanType {
    Full,
    Quick,
    CodeOnly,
    DependenciesOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityReport {
    pub scan_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub path: PathBuf,
    pub scan_type: String,
    pub vulnerabilities: Vec<Vulnerability>,
    pub summary: ScanSummary,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub total_vulnerabilities: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub info_count: usize,
    pub files_scanned: usize,
    pub scan_duration_ms: u64,
}

impl ScanSummary {
    pub fn new() -> Self {
        Self {
            total_vulnerabilities: 0,
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
            info_count: 0,
            files_scanned: 0,
            scan_duration_ms: 0,
        }
    }

    pub fn add_vulnerability(&mut self, severity: &Severity) {
        self.total_vulnerabilities += 1;
        match severity {
            Severity::Critical => self.critical_count += 1,
            Severity::High => self.high_count += 1,
            Severity::Medium => self.medium_count += 1,
            Severity::Low => self.low_count += 1,
            Severity::Info => self.info_count += 1,
        }
    }

    pub fn risk_score(&self) -> u32 {
        self.critical_count as u32 * 10 +
        self.high_count as u32 * 7 +
        self.medium_count as u32 * 4 +
        self.low_count as u32 * 2 +
        self.info_count as u32 * 1
    }

    pub fn risk_level(&self) -> String {
        let score = self.risk_score();
        match score {
            0 => "None".to_string(),
            1..=10 => "Low".to_string(),
            11..=25 => "Medium".to_string(),
            26..=50 => "High".to_string(),
            _ => "Critical".to_string(),
        }
    }
}

impl SecurityReport {
    pub fn new(path: PathBuf, scan_type: ScanType) -> Self {
        Self {
            scan_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            path,
            scan_type: format!("{:?}", scan_type),
            vulnerabilities: Vec::new(),
            summary: ScanSummary::new(),
            recommendations: Vec::new(),
        }
    }

    pub fn add_vulnerability(&mut self, vulnerability: Vulnerability) {
        self.summary.add_vulnerability(&vulnerability.severity);
        self.vulnerabilities.push(vulnerability);
    }

    pub fn finalize(&mut self, files_scanned: usize, duration_ms: u64) {
        self.summary.files_scanned = files_scanned;
        self.summary.scan_duration_ms = duration_ms;
        self.generate_recommendations();
    }

    fn generate_recommendations(&mut self) {
        let mut recommendations = Vec::new();

        if self.summary.critical_count > 0 {
            recommendations.push("🚨 Critical vulnerabilities found! Address immediately.".to_string());
        }

        if self.summary.high_count > 0 {
            recommendations.push("⚠️ High severity issues detected. Review and fix soon.".to_string());
        }

        if self.summary.medium_count > 5 {
            recommendations.push("📋 Multiple medium-severity issues. Consider a security review.".to_string());
        }

        if self.vulnerabilities.iter().any(|v| v.category == "dependency") {
            recommendations.push("📦 Update dependencies to latest secure versions.".to_string());
        }

        if self.vulnerabilities.iter().any(|v| v.category == "secret") {
            recommendations.push("🔐 Secrets detected in code. Use environment variables or secret management.".to_string());
        }

        if self.vulnerabilities.iter().any(|v| v.category == "injection") {
            recommendations.push("💉 Input validation issues found. Implement proper sanitization.".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("✅ No significant security issues found. Good job!".to_string());
        }

        self.recommendations = recommendations;
    }

    pub fn to_markdown(&self) -> String {
        let mut markdown = format!(
            "# Security Scan Report\n\n**Scan ID:** {}\n**Timestamp:** {}\n**Path:** {}\n**Type:** {}\n\n",
            self.scan_id,
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.path.display(),
            self.scan_type
        );

        // Summary
        markdown.push_str("## Summary\n\n");
        markdown.push_str(&format!("- **Total Vulnerabilities:** {}\n", self.summary.total_vulnerabilities));
        markdown.push_str(&format!("- **Risk Level:** {}\n", self.summary.risk_level()));
        markdown.push_str(&format!("- **Risk Score:** {}\n", self.summary.risk_score()));
        markdown.push_str(&format!("- **Files Scanned:** {}\n", self.summary.files_scanned));
        markdown.push_str(&format!("- **Scan Duration:** {}ms\n\n", self.summary.scan_duration_ms));

        // Severity breakdown
        if self.summary.total_vulnerabilities > 0 {
            markdown.push_str("### Severity Breakdown\n\n");
            if self.summary.critical_count > 0 {
                markdown.push_str(&format!("- 🚨 **Critical:** {}\n", self.summary.critical_count));
            }
            if self.summary.high_count > 0 {
                markdown.push_str(&format!("- ⚠️ **High:** {}\n", self.summary.high_count));
            }
            if self.summary.medium_count > 0 {
                markdown.push_str(&format!("- 📋 **Medium:** {}\n", self.summary.medium_count));
            }
            if self.summary.low_count > 0 {
                markdown.push_str(&format!("- 📝 **Low:** {}\n", self.summary.low_count));
            }
            if self.summary.info_count > 0 {
                markdown.push_str(&format!("- ℹ️ **Info:** {}\n", self.summary.info_count));
            }
            markdown.push('\n');
        }

        // Recommendations
        markdown.push_str("## Recommendations\n\n");
        for rec in &self.recommendations {
            markdown.push_str(&format!("- {}\n", rec));
        }
        markdown.push('\n');

        // Vulnerabilities
        if !self.vulnerabilities.is_empty() {
            markdown.push_str("## Vulnerabilities\n\n");
            for (i, vuln) in self.vulnerabilities.iter().enumerate() {
                markdown.push_str(&format!("### {} - {}\n\n", i + 1, vuln.title));
                markdown.push_str(&format!("- **Severity:** {:?}\n", vuln.severity));
                markdown.push_str(&format!("- **Category:** {}\n", vuln.category));
                markdown.push_str(&format!("- **File:** {}:{}\n", vuln.file_path, vuln.line_number.unwrap_or(0)));
                markdown.push_str(&format!("- **Description:** {}\n", vuln.description));
                
                if let Some(fix) = &vuln.suggested_fix {
                    markdown.push_str(&format!("- **Suggested Fix:** {}\n", fix));
                }
                
                if !vuln.references.is_empty() {
                    markdown.push_str("- **References:**\n");
                    for ref_url in &vuln.references {
                        markdown.push_str(&format!("  - {}\n", ref_url));
                    }
                }
                markdown.push('\n');
            }
        }

        markdown
    }
}
