use crate::api::{ApiClient, Cask, FetchResult, Formula};
use crate::cache::{Cache, CacheMetadata};
use crate::error::Result;
use crate::signal::check_cancelled;
use crate::tap::TapManager;
use crate::ui::create_spinner;
use console::style;
use tracing::instrument;

async fn process_formulae(cache: &Cache, fetch: &mut FetchResult<Vec<Formula>>) -> Result<usize> {
    if fetch.not_modified {
        let cached = cache.load_formulae().await?;
        Ok(cached.len())
    } else if let Some(data) = fetch.data.take() {
        let count = data.len();
        cache.save_formulae(&data).await?;
        Ok(count)
    } else {
        let cached = cache.load_formulae().await?;
        Ok(cached.len())
    }
}

async fn save_new_metadata(
    cache: &Cache,
    old_metadata: Option<&CacheMetadata>,
    formula_count: usize,
    cask_count: usize,
    formulae_fetch: &FetchResult<Vec<Formula>>,
    casks_fetch: &FetchResult<Vec<Cask>>,
) -> Result<()> {
    let new_metadata = CacheMetadata {
        last_updated: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        formula_count,
        cask_count,
        formulae_etag: formulae_fetch
            .etag
            .clone()
            .or_else(|| old_metadata.and_then(|m| m.formulae_etag.clone())),
        formulae_last_modified: formulae_fetch
            .last_modified
            .clone()
            .or_else(|| old_metadata.and_then(|m| m.formulae_last_modified.clone())),
        casks_etag: casks_fetch
            .etag
            .clone()
            .or_else(|| old_metadata.and_then(|m| m.casks_etag.clone())),
        casks_last_modified: casks_fetch
            .last_modified
            .clone()
            .or_else(|| old_metadata.and_then(|m| m.casks_last_modified.clone())),
    };
    cache.save_metadata(&new_metadata).await
}

async fn update_taps(cache: &Cache) -> Result<usize> {
    let mut tap_manager = TapManager::new()?;
    tap_manager.load().await?;
    let taps = tap_manager
        .list_taps()
        .iter()
        .map(|t| t.full_name.clone())
        .collect::<Vec<_>>();
    let tap_count = taps.len();

    if tap_count > 0 {
        cache.invalidate_all_tap_caches().await?;

        for tap_name in &taps {
            check_cancelled()?;
            if let Err(e) = tap_manager.update_tap(tap_name).await {
                eprintln!(
                    "  {} failed to update tap {}: {}",
                    style("!").yellow(),
                    style(tap_name).magenta(),
                    e
                );
            }
        }
    }

    Ok(tap_count)
}

async fn process_casks(cache: &Cache, fetch: &mut FetchResult<Vec<Cask>>) -> Result<usize> {
    if fetch.not_modified {
        let cached = cache.load_casks().await?;
        Ok(cached.len())
    } else if let Some(data) = fetch.data.take() {
        let count = data.len();
        cache.save_casks(&data).await?;
        Ok(count)
    } else {
        let cached = cache.load_casks().await?;
        Ok(cached.len())
    }
}

fn print_update_status(
    formulae_not_modified: bool,
    casks_not_modified: bool,
    formula_count: usize,
    cask_count: usize,
    tap_count: usize,
    elapsed: std::time::Duration,
) {
    let core_status = if formulae_not_modified && casks_not_modified {
        "up to date"
    } else if formulae_not_modified {
        "updated casks"
    } else if casks_not_modified {
        "updated formulae"
    } else {
        "updated"
    };

    if tap_count > 0 {
        println!(
            "{} {} · {} formulae, {} casks, {} {}{}",
            style("✓").green(),
            core_status,
            style(formula_count).cyan(),
            style(cask_count).cyan(),
            style(tap_count).cyan(),
            if tap_count == 1 { "tap" } else { "taps" },
            crate::timing::elapsed_suffix(elapsed)
        );
    } else {
        println!(
            "{} {} · {} formulae, {} casks{}",
            style("✓").green(),
            core_status,
            style(formula_count).cyan(),
            style(cask_count).cyan(),
            crate::timing::elapsed_suffix(elapsed)
        );
    }
}

async fn fetch_indices(
    api_client: &ApiClient,
    metadata: Option<&CacheMetadata>,
) -> Result<(FetchResult<Vec<Formula>>, FetchResult<Vec<Cask>>)> {
    let (formulae_etag, formulae_last_modified) = metadata
        .as_ref()
        .map(|m| {
            (
                m.formulae_etag.as_deref(),
                m.formulae_last_modified.as_deref(),
            )
        })
        .unwrap_or((None, None));

    let (casks_etag, casks_last_modified) = metadata
        .as_ref()
        .map(|m| (m.casks_etag.as_deref(), m.casks_last_modified.as_deref()))
        .unwrap_or((None, None));

    let (formulae_result, casks_result) = tokio::join!(
        api_client.fetch_formulae_conditional(formulae_etag, formulae_last_modified),
        api_client.fetch_casks_conditional(casks_etag, casks_last_modified)
    );

    Ok((formulae_result?, casks_result?))
}

#[instrument(skip(api_client, cache))]
pub async fn update(api_client: &ApiClient, cache: &Cache) -> Result<()> {
    let spinner = create_spinner("Updating package index...");

    let start = std::time::Instant::now();

    let metadata = cache.load_metadata().await?;

    let (mut formulae_fetch, mut casks_fetch) =
        fetch_indices(api_client, metadata.as_ref()).await?;

    let formula_count = process_formulae(cache, &mut formulae_fetch).await?;
    let cask_count = process_casks(cache, &mut casks_fetch).await?;

    let tap_count = update_taps(cache).await?;

    save_new_metadata(
        cache,
        metadata.as_ref(),
        formula_count,
        cask_count,
        &formulae_fetch,
        &casks_fetch,
    )
    .await?;

    spinner.finish_and_clear();

    print_update_status(
        formulae_fetch.not_modified,
        casks_fetch.not_modified,
        formula_count,
        cask_count,
        tap_count,
        start.elapsed(),
    );

    Ok(())
}
