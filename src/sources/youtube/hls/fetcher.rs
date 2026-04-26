use super::types::Resource;
use crate::common::types::AnyResult;

pub async fn fetch_segment_into(
    client: &reqwest::Client,
    resource: &Resource,
    out: &mut Vec<u8>,
) -> AnyResult<()> {
    for attempt in 0..3 {
        let mut req = client.get(&resource.url).header("Accept", "*/*");

        if let Some(range) = &resource.range {
            let end = range.offset + range.length - 1;
            req = req.header("Range", format!("bytes={}-{}", range.offset, end));
        }

        match req.timeout(std::time::Duration::from_secs(15)).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    let bytes = res.bytes().await?;
                    out.extend_from_slice(&bytes);
                    return Ok(());
                } else {
                    if attempt < 2 {
                        tracing::warn!(
                            "HLS fetch attempt {} failed {}: {} - retrying...",
                            attempt + 1,
                            res.status(),
                            resource.url
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(500 * (attempt + 1) as u64)).await;
                    } else {
                        return Err(format!("HLS fetch failed after 3 attempts {}: {}", res.status(), resource.url).into());
                    }
                }
            }
            Err(e) => {
                if attempt < 2 {
                    tracing::warn!("HLS fetch attempt {} failed: {} - retrying...", attempt + 1, e);
                    tokio::time::sleep(std::time::Duration::from_millis(500 * (attempt + 1) as u64)).await;
                } else {
                    return Err(format!("HLS fetch failed after 3 attempts: {} - {}", e, resource.url).into());
                }
            }
        }
    }

    Err(format!("HLS fetch failed: all retry attempts exhausted for {}", resource.url).into())
}