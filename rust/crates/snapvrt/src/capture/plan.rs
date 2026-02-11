use anyhow::Result;
use tokio::sync::mpsc;

use super::job::CaptureJob;
use super::runner::{CaptureOutcome, capture_all};
use crate::config::{CaptureConfig, ResolvedRunConfig};
use crate::storybook::Storybook;

/// Plans and executes a capture run: discovery, job building, filtering, capture.
pub struct CapturePlan {
    config: CaptureConfig,
    jobs: Vec<CaptureJob>,
}

impl CapturePlan {
    /// Discover stories, build the job list (stories x viewports), filter.
    pub async fn plan(config: &ResolvedRunConfig, filter: Option<&str>) -> Result<Self> {
        let local = config.capture.chrome_url.is_none();
        let storybook = Storybook::new(&config.storybook_url, local)?;
        let stories: Vec<_> = storybook
            .discover()
            .await?
            .into_iter()
            .filter(|s| !s.is_skipped())
            .collect();

        if stories.is_empty() {
            println!("No stories found at {}", storybook.url());
            return Ok(Self {
                config: config.capture.clone(),
                jobs: Vec::new(),
            });
        }

        let viewports: Vec<_> = config
            .viewports
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let snapshot_count = stories.len() * viewports.len();
        println!(
            "Discovered {} stories, {} viewport(s), {snapshot_count} snapshots",
            stories.len(),
            viewports.len()
        );
        println!();

        let mut jobs: Vec<CaptureJob> = Vec::new();
        for story in &stories {
            for (vp_name, vp) in &viewports {
                jobs.push(CaptureJob {
                    source: config.source_name.clone(),
                    story: story.clone(),
                    viewport: vp_name.clone(),
                    url: storybook.story_url(story),
                    width: vp.width,
                    height: vp.height,
                });
            }
        }

        if let Some(pattern) = filter {
            jobs.retain(|job| job.matches_filter(pattern));
            if jobs.is_empty() {
                println!("No snapshots match filter");
            }
        }

        Ok(Self {
            config: config.capture.clone(),
            jobs,
        })
    }

    pub fn total(&self) -> usize {
        self.jobs.len()
    }

    /// Return the snapshot names (IDs) for all jobs in this run.
    pub fn job_names(&self) -> Vec<String> {
        self.jobs.iter().map(|j| j.snapshot_id()).collect()
    }

    /// Launch Chrome and start capturing. Consumes self.
    pub async fn execute(self) -> Result<mpsc::Receiver<(CaptureJob, CaptureOutcome)>> {
        capture_all(self.jobs, &self.config).await
    }
}
