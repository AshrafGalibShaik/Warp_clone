use super::{SecurityConfig, SecurityReport, SecurityScanRequest, ScanType};
use super::bandit::BanditScanner;
use super::semgrep::SemgrepScanner;
use super::osv::OsvScanner;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub title: String,
    pub description: String,
    pub severity: Severity,
    pub category: String,
    pub file_path: String,
    pub line_number: Option<usize>,
    pub column_number: Option<usize>,
    pub code_snippet: Option<String>,
    pub suggested_fix: Option<String>,
    pub references: Vec<String>,
    pub scanner: String,
}

impl Vulnerability {
    pub fn new(
        title: String,
        description: String,
        severity: Severity,
        category: String,
        file_path: String,
        scanner: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            description,
            severity,
            category,
            file_path,
            line_number: None,
            column_number: None,
            code_snippet: None,
            suggested_fix: None,
            references: Vec::new(),
            scanner,
        }
    }

    pub fn with_location(mut self, line: usize, column: Option<usize>) -> Self {
        self.line_number = Some(line);
        self.column_number = column;
        self
    }

    pub fn with_code_snippet(mut self, snippet: String) -> Self {
        self.code_snippet = Some(snippet);
        self
    }

    pub fn with_fix(mut self, fix: String) -> Self {
        self.suggested_fix = Some(fix);
        self
    }

    pub fn with_references(mut self, refs: Vec<String>) -> Self {
        self.references = refs;
        self
    }
}

#[derive(Debug, Clone)]
pub enum ScanResult {
    Success(Vec<Vulnerability>),
    Error(String),
    Timeout,
}

pub struct SecurityScanner {
    config: SecurityConfig,
    bandit_scanner: Option<BanditScanner>,
    semgrep_scanner: Option<SemgrepScanner>,
    osv_scanner: Option<OsvScanner>,
}

impl SecurityScanner {
    pub fn new(config: SecurityConfig) -> Result<Self> {
        let bandit_scanner = if config.enable_bandit {
            match BanditScanner::new() {
                Ok(scanner) => Some(scanner),
                Err(e) => {
                    warn!("Failed to initialize Bandit scanner: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let semgrep_scanner = if config.enable_semgrep {
            match SemgrepScanner::new() {
                Ok(scanner) => Some(scanner),
                Err(e) => {
                    warn!("Failed to initialize Semgrep scanner: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let osv_scanner = if config.enable_osv {
            match OsvScanner::new() {
                Ok(scanner) => Some(scanner),
                Err(e) => {
                    warn!("Failed to initialize OSV scanner: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            bandit_scanner,
            semgrep_scanner,
            osv_scanner,
        })
    }

    pub async fn scan(&self, request: SecurityScanRequest) -> Result<SecurityReport> {
        let start_time = Instant::now();
        info!("Starting security scan of: {}", request.path.display());

        let mut report = SecurityReport::new(request.path.clone(), request.scan_type.clone());
        let mut files_scanned = 0;

        // Validate path exists
        if !request.path.exists() {
            return Err(anyhow!("Path does not exist: {}", request.path.display()));
        }

        // Run scans based on type and configuration
        match request.scan_type {
            ScanType::Full => {
                files_scanned += self.run_all_scanners(&request, &mut report).await?;
            }
            ScanType::Quick => {
                files_scanned += self.run_quick_scan(&request, &mut report).await?;
            }
            ScanType::CodeOnly => {
                files_scanned += self.run_code_scanners(&request, &mut report).await?;
            }
            ScanType::DependenciesOnly => {
                files_scanned += self.run_dependency_scanners(&request, &mut report).await?;
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        report.finalize(files_scanned, duration_ms);

        info!(
            "Security scan completed in {}ms. Found {} vulnerabilities.",
            duration_ms, report.summary.total_vulnerabilities
        );

        Ok(report)
    }

    async fn run_all_scanners(
        &self,
        request: &SecurityScanRequest,
        report: &mut SecurityReport,
    ) -> Result<usize> {
        let mut total_files = 0;

        // Run Bandit for Python files
        if let Some(bandit) = &self.bandit_scanner {
            match timeout(
                Duration::from_secs(self.config.scan_timeout_seconds),
                bandit.scan(&request.path),
            ).await {
                Ok(Ok(result)) => {
                    match result {
                        ScanResult::Success(vulns) => {
                            for vuln in vulns {
                                report.add_vulnerability(vuln);
                            }
                            total_files += 1;
                        }
                        ScanResult::Error(e) => {
                            warn!("Bandit scan error: {}", e);
                        }
                        ScanResult::Timeout => {
                            warn!("Bandit scan timed out");
                        }
                    }
                }
                Ok(Err(e)) => warn!("Bandit scan failed: {}", e),
                Err(_) => warn!("Bandit scan timed out"),
            }
        }

        // Run Semgrep for multiple languages
        if let Some(semgrep) = &self.semgrep_scanner {
            match timeout(
                Duration::from_secs(self.config.scan_timeout_seconds),
                semgrep.scan(&request.path),
            ).await {
                Ok(Ok(result)) => {
                    match result {
                        ScanResult::Success(vulns) => {
                            for vuln in vulns {
                                report.add_vulnerability(vuln);
                            }
                            total_files += 1;
                        }
                        ScanResult::Error(e) => {
                            warn!("Semgrep scan error: {}", e);
                        }
                        ScanResult::Timeout => {
                            warn!("Semgrep scan timed out");
                        }
                    }
                }
                Ok(Err(e)) => warn!("Semgrep scan failed: {}", e),
                Err(_) => warn!("Semgrep scan timed out"),
            }
        }

        // Run OSV for dependency vulnerabilities
        if let Some(osv) = &self.osv_scanner {
            match timeout(
                Duration::from_secs(self.config.scan_timeout_seconds),
                osv.scan(&request.path),
            ).await {
                Ok(Ok(result)) => {
                    match result {
                        ScanResult::Success(vulns) => {
                            for vuln in vulns {
                                report.add_vulnerability(vuln);
                            }
                            total_files += 1;
                        }
                        ScanResult::Error(e) => {
                            warn!("OSV scan error: {}", e);
                        }
                        ScanResult::Timeout => {
                            warn!("OSV scan timed out");
                        }
                    }
                }
                Ok(Err(e)) => warn!("OSV scan failed: {}", e),
                Err(_) => warn!("OSV scan timed out"),
            }
        }

        Ok(total_files)
    }

    async fn run_quick_scan(
        &self,
        request: &SecurityScanRequest,
        report: &mut SecurityReport,
    ) -> Result<usize> {
        // Quick scan prioritizes speed - run only essential checks
        let mut total_files = 0;

        // Run OSV first (fastest, most critical for dependencies)
        if let Some(osv) = &self.osv_scanner {
            if let Ok(result) = osv.scan(&request.path).await {
                if let ScanResult::Success(vulns) = result {
                    for vuln in vulns {
                        report.add_vulnerability(vuln);
                    }
                    total_files += 1;
                }
            }
        }

        // Run basic Semgrep rules
        if let Some(semgrep) = &self.semgrep_scanner {
            if let Ok(result) = semgrep.quick_scan(&request.path).await {
                if let ScanResult::Success(vulns) = result {
                    for vuln in vulns {
                        report.add_vulnerability(vuln);
                    }
                    total_files += 1;
                }
            }
        }

        Ok(total_files)
    }

    async fn run_code_scanners(
        &self,
        request: &SecurityScanRequest,
        report: &mut SecurityReport,
    ) -> Result<usize> {
        let mut total_files = 0;

        // Run Bandit for Python
        if let Some(bandit) = &self.bandit_scanner {
            if let Ok(result) = bandit.scan(&request.path).await {
                if let ScanResult::Success(vulns) = result {
                    for vuln in vulns {
                        report.add_vulnerability(vuln);
                    }
                    total_files += 1;
                }
            }
        }

        // Run Semgrep for multiple languages
        if let Some(semgrep) = &self.semgrep_scanner {
            if let Ok(result) = semgrep.scan(&request.path).await {
                if let ScanResult::Success(vulns) = result {
                    for vuln in vulns {
                        report.add_vulnerability(vuln);
                    }
                    total_files += 1;
                }
            }
        }

        Ok(total_files)
    }

    async fn run_dependency_scanners(
        &self,
        request: &SecurityScanRequest,
        report: &mut SecurityReport,
    ) -> Result<usize> {
        let mut total_files = 0;

        // Run OSV for dependency vulnerabilities
        if let Some(osv) = &self.osv_scanner {
            if let Ok(result) = osv.scan(&request.path).await {
                if let ScanResult::Success(vulns) = result {
                    for vuln in vulns {
                        report.add_vulnerability(vuln);
                    }
                    total_files += 1;
                }
            }
        }

        Ok(total_files)
    }

    pub fn is_scanner_available(&self, scanner_name: &str) -> bool {
        match scanner_name {
            "bandit" => self.bandit_scanner.is_some(),
            "semgrep" => self.semgrep_scanner.is_some(),
            "osv" => self.osv_scanner.is_some(),
            _ => false,
        }
    }

    pub fn get_available_scanners(&self) -> Vec<String> {
        let mut scanners = Vec::new();
        if self.bandit_scanner.is_some() {
            scanners.push("bandit".to_string());
        }
        if self.semgrep_scanner.is_some() {
            scanners.push("semgrep".to_string());
        }
        if self.osv_scanner.is_some() {
            scanners.push("osv".to_string());
        }
        scanners
    }

    pub fn update_config(&mut self, config: SecurityConfig) {
        self.config = config;
    }

    pub fn get_file_patterns_for_scan_type(scan_type: &ScanType) -> Vec<String> {
        match scan_type {
            ScanType::Full => vec![
                "*.py".to_string(),
                "*.js".to_string(),
                "*.ts".to_string(),
                "*.java".to_string(),
                "*.go".to_string(),
                "*.rs".to_string(),
                "*.php".to_string(),
                "*.rb".to_string(),
                "*.cs".to_string(),
                "*.cpp".to_string(),
                "*.c".to_string(),
                "package.json".to_string(),
                "requirements.txt".to_string(),
                "Cargo.toml".to_string(),
                "go.mod".to_string(),
                "pom.xml".to_string(),
            ],
            ScanType::Quick => vec![
                "package.json".to_string(),
                "requirements.txt".to_string(),
                "Cargo.toml".to_string(),
                "*.py".to_string(),
                "*.js".to_string(),
            ],
            ScanType::CodeOnly => vec![
                "*.py".to_string(),
                "*.js".to_string(),
                "*.ts".to_string(),
                "*.java".to_string(),
                "*.go".to_string(),
                "*.rs".to_string(),
                "*.php".to_string(),
                "*.rb".to_string(),
                "*.cs".to_string(),
                "*.cpp".to_string(),
                "*.c".to_string(),
            ],
            ScanType::DependenciesOnly => vec![
                "package.json".to_string(),
                "requirements.txt".to_string(),
                "Cargo.toml".to_string(),
                "go.mod".to_string(),
                "pom.xml".to_string(),
                "composer.json".to_string(),
                "Gemfile".to_string(),
            ],
        }
    }
}
