use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeadsIssue {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default = "default_issue_type")]
    pub issue_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    #[serde(default)]
    pub dependency_count: u32,
    pub dependencies: Option<Vec<BeadsDependency>>,
}

fn default_priority() -> u8 {
    2
}
fn default_issue_type() -> String {
    "task".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeadsDependency {
    /// When bd show returns the depended-on issue inline, this is its id.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub issue_id: Option<String>,
    #[serde(default)]
    pub depends_on_id: Option<String>,
    #[serde(rename = "type", alias = "dependency_type", default)]
    pub dep_type: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

pub struct BeadsCli {
    working_dir: PathBuf,
}

impl BeadsCli {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    pub fn from_cwd() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    fn run_bd(&self, args: &[&str]) -> Result<serde_json::Value> {
        let output = Command::new("bd")
            .args(args)
            .current_dir(&self.working_dir)
            .output()
            .context(
                "bd (beads) CLI not found. Install from https://github.com/steveyegge/beads",
            )?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            bail!(
                "bd exited with {}: {}{}",
                output.status,
                stderr.trim(),
                if stdout.trim().is_empty() {
                    String::new()
                } else {
                    format!("\nstdout: {}", stdout.trim())
                }
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(trimmed)
            .with_context(|| format!("failed to parse bd output as JSON: {trimmed}"))
    }

    pub fn check_available(&self) -> Result<String> {
        let output = Command::new("bd")
            .arg("--version")
            .current_dir(&self.working_dir)
            .output()
            .context(
                "bd (beads) CLI not found. Install from https://github.com/steveyegge/beads",
            )?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    pub fn beads_dir_exists(&self) -> bool {
        self.working_dir.join(".beads").is_dir()
    }

    pub fn list(&self, status: Option<&str>) -> Result<Vec<BeadsIssue>> {
        let mut args = vec!["list", "--json"];
        let status_flag;
        if let Some(s) = status {
            status_flag = format!("--status={s}");
            args.push(&status_flag);
        }
        let val = self.run_bd(&args)?;
        parse_issue_list(val)
    }

    pub fn show(&self, issue_id: &str) -> Result<BeadsIssue> {
        let val = self.run_bd(&["show", issue_id, "--json"])?;
        if let Some(arr) = val.as_array() {
            let first = arr
                .first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("bd show returned empty array for {issue_id}"))?;
            return serde_json::from_value(first).context("failed to parse bd show output");
        }
        serde_json::from_value(val).context("failed to parse bd show output")
    }

    pub fn ready(&self) -> Result<Vec<BeadsIssue>> {
        let val = self.run_bd(&["ready", "--json"])?;
        parse_issue_list(val)
    }

    pub fn create(
        &self,
        title: &str,
        issue_type: &str,
        priority: u8,
        parent: Option<&str>,
    ) -> Result<BeadsIssue> {
        let p_str = priority.to_string();
        let mut args = vec!["create", title, "-t", issue_type, "-p", &p_str, "--json"];
        if let Some(parent_id) = parent {
            args.push("--parent");
            args.push(parent_id);
        }
        let val = self.run_bd(&args)?;
        serde_json::from_value(val).context("failed to parse bd create output")
    }

    pub fn update_status(&self, issue_id: &str, status: &str) -> Result<BeadsIssue> {
        let val = self.run_bd(&["update", issue_id, "--status", status, "--json"])?;
        serde_json::from_value(val).context("failed to parse bd update output")
    }

    pub fn close(&self, issue_id: &str, reason: Option<&str>) -> Result<BeadsIssue> {
        let mut args = vec!["close", issue_id, "--json"];
        if let Some(r) = reason {
            args.push("--reason");
            args.push(r);
        }
        let val = self.run_bd(&args)?;
        serde_json::from_value(val).context("failed to parse bd close output")
    }

    pub fn add_dependency(&self, issue_id: &str, depends_on_id: &str) -> Result<()> {
        self.run_bd(&["dep", "add", issue_id, depends_on_id])?;
        Ok(())
    }
}

fn parse_issue_list(val: serde_json::Value) -> Result<Vec<BeadsIssue>> {
    if val.is_null() {
        return Ok(vec![]);
    }
    if val.is_array() {
        return serde_json::from_value(val).context("failed to parse bd list output");
    }
    // bd may wrap the list in an object
    if let Some(issues) = val.get("issues") {
        return serde_json::from_value(issues.clone()).context("failed to parse bd list output");
    }
    serde_json::from_value(val).context("failed to parse bd list output")
}

/// Determine which beads issues are children of a given epic.
/// A child is a task whose parent is the epic (via bd --parent) or that has a
/// dependency of type "blocks" where depends_on_id == epic_id.
pub fn children_of_epic(issues: &[BeadsIssue], epic_id: &str) -> Vec<BeadsIssue> {
    issues
        .iter()
        .filter(|i| {
            i.dependencies.as_ref().map_or(false, |deps| {
                deps.iter().any(|d| dependency_refs_epic(d, epic_id))
            })
        })
        .cloned()
        .collect()
}

/// Check if an issue ID looks like a hierarchical child (e.g. "epic-id.1").
pub fn is_hierarchical_child(issue_id: &str, epic_id: &str) -> bool {
    issue_id.starts_with(&format!("{epic_id}."))
}

/// True if this dependency references the given epic (id or depends_on_id).
pub fn dependency_refs_epic(d: &BeadsDependency, epic_id: &str) -> bool {
    d.id.as_deref() == Some(epic_id) || d.depends_on_id.as_deref() == Some(epic_id)
}

/// Get children including hierarchical IDs and dependency references.
pub fn all_children(issues: &[BeadsIssue], epic_id: &str) -> Vec<BeadsIssue> {
    issues
        .iter()
        .filter(|i| {
            is_hierarchical_child(&i.id, epic_id)
                || i.dependencies.as_ref().map_or(false, |deps| {
                    deps.iter().any(|d| dependency_refs_epic(d, epic_id))
                })
        })
        .cloned()
        .collect()
}
