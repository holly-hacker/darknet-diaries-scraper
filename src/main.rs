use std::{path::PathBuf, time::Duration};

use regex::Regex;
use scraper::{Html, Selector};
use tokio::time::sleep;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    result_main().await.unwrap();
}

async fn result_main() -> anyhow::Result<()> {
    let latest_ep = find_latest_episode().await?;
    println!("Latest episode: {latest_ep}");

    let mut tasks = vec![];
    const MAX_TASK_COUNT: usize = 8;

    let mut path = PathBuf::new();
    path.push("transcripts");
    tokio::fs::create_dir_all(&path).await?;

    for idx in 1..=latest_ep {
        let handle = tokio::spawn(async move {
            let transcript = match download_transcript(idx).await {
                Ok(s) => s,
                _ => {
                    println!("failed to find transcript for episode {idx}");
                    return;
                }
            };

            let mut path = PathBuf::new();
            path.push("transcripts");
            path.push(format!("ep{idx}.txt"));
            if let Err(x) = tokio::fs::write(&path, &transcript).await {
                println!("Error trying to write file: {x}");
                return;
            };

            println!("Downloaded episode {idx}");
        });

        tasks.push(handle);

        while tasks.len() >= (MAX_TASK_COUNT as usize) {
            // sleep
            sleep(Duration::from_millis(1)).await;

            // remove dead threads
            tasks.retain(|h| !h.is_finished());
        }
    }

    // wait for remaining tasks to finish
    while !tasks.is_empty() {
        sleep(Duration::from_millis(1)).await;

        // remove dead threads
        tasks.retain(|h| !h.is_finished());
    }

    println!("Done!");

    Ok(())
}

async fn find_latest_episode() -> anyhow::Result<u32> {
    let response = reqwest::get("https://darknetdiaries.com/episode/").await?;
    let source = response.text().await?;

    let x = Regex::new("/episode/(\\d+)/")?;
    let stuff = x
        .captures_iter(&source)
        .flat_map(|x| x.get(1))
        .flat_map(|x| x.as_str().parse())
        .max();

    let newest_episode = match stuff {
        Some(i) => i,
        _ => anyhow::bail!("Could not find latest episode number"),
    };

    Ok(newest_episode)
}

async fn download_transcript(idx: u32) -> anyhow::Result<String> {
    let response = reqwest::get(format!("https://darknetdiaries.com/transcript/{idx}/")).await?;
    let source = response.text().await?;

    let document = Html::parse_document(&source);

    let selector = Selector::parse(".single-post>pre").expect("Failed to parse selector");
    let element = document.select(&selector).next();
    let element = element.ok_or_else(|| anyhow::anyhow!("Failed to find transcription in html"))?;

    let text = element.text().collect();

    Ok(text)
}
