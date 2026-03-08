use std::time::Instant;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::warn;

use super::job::CaptureJob;
use super::runner::{CaptureOutcome, capture_all};
use super::timing::CaptureTimings;
use crate::config::resolve::ResolvedSource;
use crate::config::{CaptureConfig, ResolvedRunConfig};
use crate::storybook::{Story, Storybook};
use crate::{store, typst};

/// Internal representation: Chrome-based jobs or pre-rendered Typst results.
enum CaptureKind {
    Chrome {
        config: CaptureConfig,
        jobs: Vec<CaptureJob>,
    },
    Precomputed {
        results: Vec<(CaptureJob, CaptureOutcome)>,
    },
}

/// Plans and executes a capture run: discovery, job building, filtering, capture.
pub struct CapturePlan {
    kind: CaptureKind,
}

impl CapturePlan {
    /// Discover stories/templates, build the job list, filter.
    pub async fn plan(config: &ResolvedRunConfig, filter: Option<&str>) -> Result<Self> {
        match &config.source {
            ResolvedSource::Storybook { url } => Self::plan_storybook(config, url, filter).await,
            ResolvedSource::Typst {
                root,
                include,
                scale,
                pdf,
            } => Self::plan_typst(config, root, include, *scale, *pdf, filter).await,
        }
    }

    async fn plan_storybook(
        config: &ResolvedRunConfig,
        url: &str,
        filter: Option<&str>,
    ) -> Result<Self> {
        let local = config.capture.chrome_url.is_none();
        let storybook = Storybook::new(url, local)?;
        let stories: Vec<_> = storybook
            .discover()
            .await?
            .into_iter()
            .filter(|s| !s.is_skipped())
            .collect();

        if stories.is_empty() {
            println!("No stories found at {}", storybook.url());
            return Ok(Self {
                kind: CaptureKind::Chrome {
                    config: config.capture.clone(),
                    jobs: Vec::new(),
                },
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
            kind: CaptureKind::Chrome {
                config: config.capture.clone(),
                jobs,
            },
        })
    }

    async fn plan_typst(
        config: &ResolvedRunConfig,
        root: &std::path::Path,
        include: &[String],
        scale: f32,
        pdf: bool,
        filter: Option<&str>,
    ) -> Result<Self> {
        let templates = typst::discover(include)?;

        if templates.is_empty() {
            println!("No Typst templates found");
            return Ok(Self {
                kind: CaptureKind::Precomputed {
                    results: Vec::new(),
                },
            });
        }

        let fixture_count: usize = templates.iter().map(|t| t.fixtures.len().max(1)).sum();
        println!(
            "Discovered {} Typst template(s), {} compile target(s)",
            templates.len(),
            fixture_count,
        );
        println!();

        let mut results: Vec<(CaptureJob, CaptureOutcome)> = Vec::new();

        for template in &templates {
            if template.fixtures.is_empty() {
                // Self-contained template: compile once, no fixture
                let pdf_path = if pdf {
                    let pdf_id = format!("{}/default/{}", config.source_name, template.stem);
                    Some(store::pdf_path(&pdf_id))
                } else {
                    None
                };

                let opts = typst::CompileOptions {
                    root,
                    template: &template.path,
                    fixture: None,
                    scale,
                    pdf_path,
                };

                compile_template(
                    &opts,
                    &config.source_name,
                    &template.stem,
                    None,
                    filter,
                    &mut results,
                )
                .await;
            } else {
                // Template with fixtures: compile once per fixture
                for fixture in &template.fixtures {
                    let pdf_path = if pdf {
                        let pdf_id = format!(
                            "{}/default/{}/{}",
                            config.source_name, template.stem, fixture.name
                        );
                        Some(store::pdf_path(&pdf_id))
                    } else {
                        None
                    };

                    let opts = typst::CompileOptions {
                        root,
                        template: &template.path,
                        fixture: Some(fixture),
                        scale,
                        pdf_path,
                    };

                    compile_template(
                        &opts,
                        &config.source_name,
                        &template.stem,
                        Some(&fixture.name),
                        filter,
                        &mut results,
                    )
                    .await;
                }
            }
        }

        if filter.is_some() && results.is_empty() {
            println!("No snapshots match filter");
        } else {
            println!(
                "{} snapshot(s) from {} template(s)",
                results.len(),
                templates.len()
            );
            println!();
        }

        Ok(Self {
            kind: CaptureKind::Precomputed { results },
        })
    }

    pub fn total(&self) -> usize {
        match &self.kind {
            CaptureKind::Chrome { jobs, .. } => jobs.len(),
            CaptureKind::Precomputed { results } => results.len(),
        }
    }

    /// Return the snapshot names (IDs) for all jobs in this run.
    pub fn job_names(&self) -> Vec<String> {
        match &self.kind {
            CaptureKind::Chrome { jobs, .. } => jobs.iter().map(|j| j.snapshot_id()).collect(),
            CaptureKind::Precomputed { results } => {
                results.iter().map(|(j, _)| j.snapshot_id()).collect()
            }
        }
    }

    /// Launch capture. Consumes self.
    pub async fn execute(self) -> Result<mpsc::Receiver<(CaptureJob, CaptureOutcome)>> {
        match self.kind {
            CaptureKind::Chrome { jobs, config } => capture_all(jobs, &config).await,
            CaptureKind::Precomputed { results } => {
                let (tx, rx) = mpsc::channel(results.len().max(1));
                for item in results {
                    let _ = tx.send(item).await;
                }
                Ok(rx)
            }
        }
    }
}

/// Compile a single template (with or without fixture) and push results.
async fn compile_template(
    opts: &typst::CompileOptions<'_>,
    source_name: &str,
    template_stem: &str,
    fixture_name: Option<&str>,
    filter: Option<&str>,
    results: &mut Vec<(CaptureJob, CaptureOutcome)>,
) {
    let t_start = Instant::now();
    let label = match fixture_name {
        Some(f) => format!("{template_stem}/{f}"),
        None => template_stem.to_string(),
    };

    match typst::compile(opts).await {
        Ok(pages) => {
            let elapsed = t_start.elapsed();
            let page_count = pages.len();
            for page in pages {
                let page_name = if page_count == 1 {
                    "page".to_string()
                } else {
                    format!("page_{}", page.page)
                };

                // Build the title: stem or stem/fixture
                let title = match fixture_name {
                    Some(f) => format!("{template_stem}/{f}"),
                    None => template_stem.to_string(),
                };

                let story = Story {
                    id: format!("{title}/{page_name}"),
                    name: page_name,
                    title,
                    tags: vec![],
                };
                let job = CaptureJob {
                    source: source_name.to_string(),
                    story,
                    viewport: "default".to_string(),
                    url: String::new(),
                    width: 0,
                    height: 0,
                };

                let should_include = filter.map(|p| job.matches_filter(p)).unwrap_or(true);

                if should_include {
                    let timings = CaptureTimings {
                        total: elapsed / page_count as u32,
                        ..CaptureTimings::zero()
                    };
                    results.push((job, CaptureOutcome::Ok(page.png, timings)));
                }
            }
        }
        Err(e) => {
            warn!(
                label = %label,
                error = %format!("{e:#}"),
                "typst compile failed"
            );
            let story = Story {
                id: label.clone(),
                name: "page".to_string(),
                title: label,
                tags: vec![],
            };
            let job = CaptureJob {
                source: source_name.to_string(),
                story,
                viewport: "default".to_string(),
                url: String::new(),
                width: 0,
                height: 0,
            };
            results.push((job, CaptureOutcome::Err(format!("{e:#}"))));
        }
    }
}
