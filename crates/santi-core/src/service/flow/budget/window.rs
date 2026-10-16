use super::*;

impl Service {
    pub(in crate::service) async fn held(&self, strand: &str) -> Result<Option<Fault>, String> {
        let key = crate::budget::Error::Context
            .descriptor()
            .key("strand", strand);
        Ok(self
            .store
            .incident(&key)
            .await?
            .filter(|incident| incident.latest.context["details"]["maintenance_pause"] == true)
            .map(|incident| crate::engine().fault(&incident)))
    }

    pub(in crate::service) fn suffix(&self, request: &mut santi_provider::Request) {
        let Some(cap) = self.budget() else {
            return;
        };
        let index = request.input.len();
        request.input.push(santi_provider::Item::Message {
            role: "system".into(),
            content: suffix(0, &cap),
        });
        let used = crate::context::budget::gauged(request).total;
        request.input[index] = santi_provider::Item::Message {
            role: "system".into(),
            content: suffix(used, &cap),
        };
    }
}

fn suffix(used: i64, cap: &budget::Cap) -> String {
    format!(
        "<system_message>\nkind: context_budget\nnormal_bytes: {:020}\nmaintenance_bytes: {:020}\nused_bytes: {:020}\nremaining_bytes: {:020}\nstate: Context maintenance offers query and compact. Exceeding the maintenance byte allowance pauses this strand until explicit recovery; inbox input remains stored.\n</system_message>",
        cap.bytes.max(0),
        cap.ceiling().max(0),
        used.max(0),
        cap.ceiling().saturating_sub(used).max(0)
    )
}
