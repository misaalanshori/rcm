use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

use crate::client::RcClient;
use crate::error::RcError;
use crate::types::{OAuthStatusResponse, OptionSchema, WizardStepResponse};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WizardStep {
    AskQuestion {
        state: String,
        option: Box<OptionSchema>,
        error: Option<String>,
    },
    OAuthInProgress {
        auth_url: Option<String>,
    },
    Completed {
        remote_name: String,
    },
    Failed {
        error: String,
    },
}

#[derive(Clone)]
pub struct WizardDriver {
    client: RcClient,
    remote_name: String,
    backend_type: String,
    all_questions: bool,
    default_parameters: BTreeMap<String, serde_json::Value>,
    current_state: String,
}

impl WizardDriver {
    pub fn new(
        client: RcClient,
        remote_name: impl Into<String>,
        backend_type: impl Into<String>,
        preset_parameters: BTreeMap<String, serde_json::Value>,
        all_questions: bool,
    ) -> Self {
        Self {
            client,
            remote_name: remote_name.into(),
            backend_type: backend_type.into(),
            all_questions,
            default_parameters: preset_parameters,
            current_state: String::new(),
        }
    }

    pub fn remote_name(&self) -> &str {
        &self.remote_name
    }

    pub fn backend_type(&self) -> &str {
        &self.backend_type
    }

    pub fn current_state(&self) -> &str {
        &self.current_state
    }

    /// Starts the wizard by calling config/create with nonInteractive=true (R2, §7.4)
    pub async fn start(&mut self) -> Result<WizardStep, RcError> {
        let mut opt = BTreeMap::new();
        opt.insert("nonInteractive".to_string(), serde_json::Value::Bool(true));
        if self.all_questions {
            opt.insert("all".to_string(), serde_json::Value::Bool(true));
        }

        let resp_value = self
            .client
            .config_create(
                &self.remote_name,
                &self.backend_type,
                self.default_parameters.clone(),
                opt,
            )
            .await?;

        self.process_response(resp_value)
    }

    /// Answers the current question by calling config/update with continue=true (R2, §7.4)
    pub async fn answer(&mut self, result: &str, obscure: bool) -> Result<WizardStep, RcError> {
        let mut opt = BTreeMap::new();
        opt.insert("continue".to_string(), serde_json::Value::Bool(true));
        opt.insert(
            "state".to_string(),
            serde_json::Value::String(self.current_state.clone()),
        );
        opt.insert(
            "result".to_string(),
            serde_json::Value::String(result.to_string()),
        );
        if obscure {
            opt.insert("obscure".to_string(), serde_json::Value::Bool(true));
        }
        if self.all_questions {
            opt.insert("all".to_string(), serde_json::Value::Bool(true));
        }

        let resp_value = self
            .client
            .config_update(
                &self.remote_name,
                self.default_parameters.clone(),
                opt,
            )
            .await?;

        self.process_response(resp_value)
    }

    /// Polls OAuth status for OAuth flow (CF-3)
    pub async fn poll_oauth(&self) -> Result<OAuthStatusResponse, RcError> {
        self.client.config_oauth_status().await
    }

    /// Cancels wizard & any in-progress OAuth token server (CF-3)
    pub async fn cancel(&self) -> Result<(), RcError> {
        let _ = self.client.config_oauth_stop().await;
        let _ = self.client.config_delete(&self.remote_name).await;
        Ok(())
    }

    fn process_response(&mut self, val: serde_json::Value) -> Result<WizardStep, RcError> {
        let step_resp: WizardStepResponse = serde_json::from_value(val.clone()).map_err(|e| {
            RcError::Wizard(format!(
                "Failed to parse wizard step response: {}. Body: {:?}",
                e, val
            ))
        })?;

        self.current_state = step_resp.state.clone();

        if step_resp.state.is_empty() {
            if !step_resp.error.is_empty() {
                return Ok(WizardStep::Failed {
                    error: step_resp.error,
                });
            }
            return Ok(WizardStep::Completed {
                remote_name: self.remote_name.clone(),
            });
        }

        // Check if OAuth question or normal question
        if let Some(opt) = step_resp.option {
            let err_opt = if step_resp.error.is_empty() {
                None
            } else {
                Some(step_resp.error)
            };
            return Ok(WizardStep::AskQuestion {
                state: step_resp.state,
                option: Box::new(opt),
                error: err_opt,
            });
        }

        // If State is non-empty and has an error but no Option
        if !step_resp.error.is_empty() {
            return Ok(WizardStep::Failed {
                error: step_resp.error,
            });
        }

        Ok(WizardStep::OAuthInProgress { auth_url: None })
    }
}
