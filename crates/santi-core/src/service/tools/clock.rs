use super::Service;

pub(super) async fn selected(service: &Service, strand: &str) -> Result<bool, String> {
    Ok(service.store.strand(strand).await?.is_some_and(|strand| {
        strand.label.as_deref() == Some(crate::service::engine::watch::clock::LABEL)
    }))
}
