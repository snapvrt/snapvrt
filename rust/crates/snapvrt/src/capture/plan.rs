use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::warn;

use super::job::CaptureJob;
use super::runner::{CaptureOutcome, capture_all};
use super::timing::CaptureTimings;
use crate::config::resolve::ResolvedSource;
use crate::config::{CaptureConfig, ResolvedRunConfig, Viewport};
use crate::storybook::{Story, Storybook};
use crate::{store, typst};

/// Plans and executes a capture run across one or more sources.
pub struct CapturePlan {
    /// Chrome-based capture jobs (storybook + pages sources).
    chrome_jobs: Vec<CaptureJob>,
    /// Pre-rendered results (typst sources).
    precomputed: Vec<(CaptureJob, CaptureOutcome)>,
    /// Capture config for Chrome jobs.
    capture_config: CaptureConfig,
}

impl CapturePlan {
    /// Discover stories/templates/pages from all configured sources, build the
    /// combined job list, and apply filter.
    pub async fn plan(config: &ResolvedRunConfig, filter: Option<&str>) -> Result<Self> {
        let mut chrome_jobs: Vec<CaptureJob> = Vec::new();
        let mut precomputed: Vec<(CaptureJob, CaptureOutcome)> = Vec::new();

        for entry in &config.sources {
            match &entry.source {
                ResolvedSource::Storybook { url } => {
                    let jobs = plan_storybook(
                        &entry.name,
                        &entry.viewports,
                        &config.capture,
                        url,
                        filter,
                    )
                    .await?;
                    chrome_jobs.extend(jobs);
                }
                ResolvedSource::Pages { base_url, pages } => {
                    let jobs =
                        plan_pages(&entry.name, &entry.viewports, base_url, pages, filter).await?;
                    chrome_jobs.extend(jobs);
                }
                ResolvedSource::Typst {
                    root,
                    include,
                    scale,
                    pdf,
                    font_paths,
                } => {
                    let results = plan_typst(
                        &entry.name,
                        root,
                        include,
                        *scale,
                        *pdf,
                        font_paths,
                        filter,
                    )
                    .await?;
                    precomputed.extend(results);
                }
            }
        }

        Ok(Self {
            chrome_jobs,
            precomputed,
            capture_config: config.capture.clone(),
        })
    }

    pub fn total(&self) -> usize {
        self.chrome_jobs.len() + self.precomputed.len()
    }

    /// Return the snapshot names (IDs) for all jobs in this run.
    pub fn job_names(&self) -> Vec<String> {
        self.chrome_jobs
            .iter()
            .map(|j| j.snapshot_id())
            .chain(self.precomputed.iter().map(|(j, _)| j.snapshot_id()))
            .collect()
    }

    /// Launch capture. Consumes self.
    ///
    /// Precomputed results are sent first, then Chrome jobs are captured.
    pub async fn execute(self) -> Result<mpsc::Receiver<(CaptureJob, CaptureOutcome)>> {
        let has_chrome = !self.chrome_jobs.is_empty();
        let has_precomputed = !self.precomputed.is_empty();

        // Only precomputed (typst-only config).
        if !has_chrome {
            let cap = self.precomputed.len().max(1);
            let (tx, rx) = mpsc::channel(cap);
            for item in self.precomputed {
                let _ = tx.send(item).await;
            }
            return Ok(rx);
        }

        // Only Chrome jobs (no typst).
        if !has_precomputed {
            return capture_all(self.chrome_jobs, &self.capture_config).await;
        }

        // Both: send precomputed first, then stream Chrome results.
        let (tx, rx) = mpsc::channel(self.total().max(1));

        // Send precomputed immediately.
        for item in self.precomputed {
            let _ = tx.send(item).await;
        }

        // Capture Chrome jobs and forward results.
        let mut chrome_rx = capture_all(self.chrome_jobs, &self.capture_config).await?;
        tokio::spawn(async move {
            while let Some(item) = chrome_rx.recv().await {
                if tx.send(item).await.is_err() {
                    break;
                }
            }
        });

        Ok(rx)
    }
}

// ---------------------------------------------------------------------------
// Per-source planning functions
// ---------------------------------------------------------------------------

async fn plan_storybook(
    source_name: &str,
    viewports: &BTreeMap<String, Viewport>,
    capture_config: &CaptureConfig,
    url: &str,
    filter: Option<&str>,
) -> Result<Vec<CaptureJob>> {
    let local = capture_config.chrome_url.is_none();
    let storybook = Storybook::new(url, local)?;
    let stories: Vec<_> = storybook
        .discover()
        .await?
        .into_iter()
        .filter(|s| !s.is_skipped())
        .collect();

    if stories.is_empty() {
        println!("No stories found at {}", storybook.url());
        return Ok(Vec::new());
    }

    let vps: Vec<_> = viewports
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let snapshot_count = stories.len() * vps.len();
    println!(
        "Discovered {} stories, {} viewport(s), {snapshot_count} snapshots",
        stories.len(),
        vps.len()
    );
    println!();

    let mut jobs = Vec::new();
    for story in &stories {
        for (vp_name, vp) in &vps {
            jobs.push(CaptureJob {
                source: source_name.to_string(),
                story: story.clone(),
                viewport: vp_name.clone(),
                url: storybook.story_url(story),
                width: vp.width,
                height: vp.height,
                full_page: false,
            });
        }
    }

    if let Some(pattern) = filter {
        jobs.retain(|job| job.matches_filter(pattern));
        if jobs.is_empty() {
            println!("No snapshots match filter");
        }
    }

    Ok(jobs)
}

async fn plan_pages(
    source_name: &str,
    viewports: &BTreeMap<String, Viewport>,
    base_url: &str,
    pages: &[String],
    filter: Option<&str>,
) -> Result<Vec<CaptureJob>> {
    let base_url = base_url.trim_end_matches('/');

    let vps: Vec<_> = viewports
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let snapshot_count = pages.len() * vps.len();
    println!(
        "{} page(s), {} viewport(s), {snapshot_count} snapshots",
        pages.len(),
        vps.len()
    );
    println!();

    let mut jobs = Vec::new();
    for page_path in pages {
        let path = page_path.trim_start_matches('/');
        let title = if path.is_empty() { "root" } else { path };
        let name = title.rsplit('/').next().unwrap_or(title);

        let story = Story {
            id: title.to_string(),
            name: name.to_string(),
            title: title.to_string(),
            tags: vec![],
        };

        for (vp_name, vp) in &vps {
            jobs.push(CaptureJob {
                source: source_name.to_string(),
                story: story.clone(),
                viewport: vp_name.clone(),
                url: format!("{base_url}/{path}"),
                width: vp.width,
                height: vp.height,
                full_page: true,
            });
        }
    }

    if let Some(pattern) = filter {
        jobs.retain(|job| job.matches_filter(pattern));
        if jobs.is_empty() {
            println!("No snapshots match filter");
        }
    }

    Ok(jobs)
}

async fn plan_typst(
    source_name: &str,
    root: &std::path::Path,
    include: &[String],
    scale: f32,
    pdf: bool,
    font_paths: &[String],
    filter: Option<&str>,
) -> Result<Vec<(CaptureJob, CaptureOutcome)>> {
    let templates = typst::discover(include)?;

    if templates.is_empty() {
        println!("No Typst templates found");
        return Ok(Vec::new());
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
            let pdf_path = if pdf {
                let pdf_id = format!("{source_name}/default/{}", template.stem);
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
                font_paths,
            };

            compile_template(&opts, source_name, &template.stem, None, filter, &mut results).await;
        } else {
            for fixture in &template.fixtures {
                let pdf_path = if pdf {
                    let pdf_id =
                        format!("{source_name}/default/{}/{}", template.stem, fixture.name);
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
                    font_paths,
                };

                compile_template(
                    &opts,
                    source_name,
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

    Ok(results)
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
                    full_page: false,
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
                full_page: false,
            };
            results.push((job, CaptureOutcome::Err(format!("{e:#}"))));
        }
    }
}
