use anyhow::Result;
use clap::Parser;

use poc_storybook_discovery::discovery;

#[derive(Parser)]
#[command(about = "Discover stories from a Storybook instance")]
struct Cli {
    /// Storybook base URL
    #[arg(long, default_value = "http://localhost:6006")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let stories = discovery::discover(&cli.url).await?;

    for story in &stories {
        println!("  {} — {} / {}", story.id, story.title, story.name);
        println!("    {}", story.url);
    }

    println!("\n{} stories discovered", stories.len());

    Ok(())
}
