use crate::Cache;
use arc_swap::ArcSwap;
use rebuilderd_common::api::{
    Client,
    v1::{PackageRestApi, Page},
};
use rebuilderd_common::errors::*;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::time::{self, Duration};

const REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 30);

async fn fetch_binary_pkgs(client: &Client) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let mut page = Page {
        limit: Some(1000),
        before: None,
        after: None,
        sort: Some("name".to_string()),
        direction: None,
    };

    let mut pkgs = BTreeSet::<String>::new();
    let mut archs = BTreeSet::<String>::new();

    loop {
        let results = client.get_binary_packages(Some(&page), None, None).await?;

        if let Some(last) = results.records.last() {
            page.after = Some(last.id);
        } else {
            break;
        }

        for result in results.records {
            pkgs.insert(result.name);
            archs.insert(result.architecture);
        }
    }

    Ok((pkgs, archs))
}

async fn fetch_source_pkgs(client: &Client) -> Result<BTreeSet<String>> {
    let mut page = Page {
        limit: Some(1000),
        before: None,
        after: None,
        sort: Some("name".to_string()),
        direction: None,
    };

    let mut pkgs = BTreeSet::<String>::new();

    loop {
        let results = client.get_source_packages(Some(&page), None, None).await?;

        if let Some(last) = results.records.last() {
            page.after = Some(last.id);
        } else {
            break;
        }

        for result in results.records {
            pkgs.insert(result.name);
        }
    }

    Ok(pkgs)
}

async fn fetch(client: &Client, cache: &ArcSwap<Cache>) -> Result<()> {
    let ((binary_pkgs, architectures), source_pkgs) =
        tokio::try_join!(fetch_binary_pkgs(client), fetch_source_pkgs(client))?;

    let new_cache = Cache {
        binary_pkgs,
        source_pkgs,
        architectures,
    };
    cache.store(Arc::new(new_cache));
    info!("Refreshed package cache");

    Ok(())
}

pub async fn run(client: Client, cache: Arc<ArcSwap<Cache>>) {
    let mut interval = time::interval(REFRESH_INTERVAL);
    loop {
        interval.tick().await;

        if let Err(err) = fetch(&client, &cache).await {
            error!("Error fetching packages: {err}");
        }
    }
}
