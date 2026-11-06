use santi_model::receipt::reply::{Attempt, Owner, Resolution, State, Window};
use serde_json::{Value, json};

pub(super) enum Change<'a> {
    Queue(super::Intent<'a>),
    Dispatch,
    Settle(super::Outcome<'a>),
    Interrupt,
    Resolve(&'a Resolution),
}

pub(super) struct Machine<'a>(pub(super) &'a mut Window);

impl Machine<'_> {
    pub(super) fn apply(&mut self, change: Change<'_>) -> Result<Option<Attempt>, String> {
        match change {
            Change::Queue(intent) => self.queue(intent.owner, intent.operation, intent.body)?,
            Change::Dispatch => return Ok(self.dispatch()),
            Change::Settle(result) => {
                self.settle(result.operation, result.result, result.message)?
            }
            Change::Interrupt => self.interrupt(),
            Change::Resolve(resolution) => self.resolve(resolution)?,
        }
        Ok(None)
    }

    fn queue(&mut self, owner: Owner, operation: &str, body: &Value) -> Result<(), String> {
        if operation.is_empty() || operation.len() > 128 {
            return Err("reply operation must contain 1..128 bytes".into());
        }
        if let Some(old) = self.0.attempts.iter().find(|a| a.operation == operation) {
            if old.owner != owner || old.body != *body {
                return Err("reply operation conflicts with its retained intent".into());
            }
            return Ok(());
        }
        if owner == Owner::Runtime {
            if self.0.owner == Owner::Soul || self.0.attempts.len() >= 16 {
                return Ok(());
            }
        } else {
            if operation.starts_with("runtime:") {
                return Err("runtime reply operation namespace is reserved".into());
            }
            if self.0.owner == Owner::Soul {
                return Err(
                    "receipt already has a soul reply; inspect its original operation".into(),
                );
            }
            self.0.owner = Owner::Soul;
        }
        for old in &mut self.0.attempts {
            if old.owner == Owner::Runtime && old.state == State::Queued {
                old.state = State::Cancelled;
            }
        }
        self.0.attempts.push(Attempt {
            operation: operation.into(),
            uuid: self.uuid(operation)?,
            owner,
            body: body.clone(),
            state: State::Queued,
            target: None,
            result: None,
            resolution: None,
        });
        Ok(())
    }

    fn dispatch(&mut self) -> Option<Attempt> {
        if self
            .0
            .attempts
            .iter()
            .any(|a| matches!(a.state, State::Dispatching | State::Unknown))
        {
            return None;
        }
        let next = self
            .0
            .attempts
            .iter_mut()
            .find(|a| a.state == State::Queued)?;
        next.target = self.0.message.clone();
        next.state = State::Dispatching;
        Some(next.clone())
    }

    fn settle(
        &mut self,
        operation: &str,
        result: &Value,
        message: Option<&str>,
    ) -> Result<(), String> {
        let attempt = self
            .0
            .attempts
            .iter_mut()
            .find(|a| a.operation == operation)
            .ok_or("reply operation not found")?;
        if attempt.state != State::Dispatching {
            return Err("reply dispatch already settled; never replay".into());
        }
        let state = match result["state"].as_str() {
            Some("accepted") => State::Accepted,
            Some("rejected") => State::Rejected,
            Some("unsent") => State::Unsent,
            Some("unknown") => State::Unknown,
            _ => return Err("invalid reply result state".into()),
        };
        if state == State::Accepted {
            let message = message
                .filter(|s| !s.is_empty())
                .ok_or("accepted reply requires exact message identity")?;
            if attempt
                .target
                .as_deref()
                .is_some_and(|target| target != message)
            {
                return Err("accepted reply identifies a foreign message".into());
            }
            self.0.message = Some(message.into());
        }
        attempt.state = state;
        attempt.result = Some(result.clone());
        Ok(())
    }

    fn resolve(&mut self, resolution: &Resolution) -> Result<(), String> {
        if resolution.evidence.trim().is_empty() {
            return Err("reply resolution requires exact remote settlement evidence; current body alone is insufficient".into());
        }
        let attempt = self
            .0
            .attempts
            .iter_mut()
            .find(|a| a.operation == resolution.operation)
            .ok_or("reply operation not found")?;
        if attempt.state != State::Unknown {
            return Err("only an unknown reply dispatch may be resolved".into());
        }
        if resolution.applied {
            let message = resolution
                .message
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or("applied reply resolution requires the observed message identity")?;
            if attempt
                .target
                .as_deref()
                .is_some_and(|target| target != message)
            {
                return Err("reply resolution identifies a foreign message".into());
            }
            self.0.message = Some(message.into());
            attempt.state = State::Observed;
        } else {
            attempt.state = State::NotApplied;
        }
        attempt.resolution = Some(serde_json::to_value(resolution).map_err(|e| e.to_string())?);
        Ok(())
    }

    fn uuid(&self, operation: &str) -> Result<String, String> {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(&(&self.0.binding, operation)).map_err(|e| e.to_string())?;
        Ok(format!("{:x}", Sha256::digest(bytes))[..32].into())
    }

    fn interrupt(&mut self) {
        for attempt in &mut self.0.attempts {
            if attempt.state == State::Dispatching {
                attempt.state = State::Unknown;
                attempt.result =
                    Some(json!({"state":"unknown", "reason":"dispatch interrupted; never replay"}));
            }
        }
    }
}
