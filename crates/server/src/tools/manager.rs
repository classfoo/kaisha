use crate::tools::{
    chat_stream,
    driver::{CodingToolDriver, ToolChatMessage, ToolExecutionResult, ToolSession},
    model::{CreateToolInstanceRequest, ToolCatalogItem, ToolInstance, ToolKind, UpdateToolInstanceRequest},
    registry::ToolRegistry,
    store::ToolStore,
};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct ToolManager {
    registry: ToolRegistry,
    store: ToolStore,
    instances: BTreeMap<String, ToolInstance>,
}

impl ToolManager {
    pub fn new(workspace: Option<&Path>) -> anyhow::Result<Self> {
        let registry = ToolRegistry::new();
        let store = ToolStore::new(workspace)?;
        let mut manager = Self {
            registry,
            store,
            instances: BTreeMap::new(),
        };
        manager.instances = manager.store.load()?;
        Ok(manager)
    }

    pub fn reload(&mut self, workspace: Option<&Path>) -> anyhow::Result<()> {
        self.store = ToolStore::new(workspace)?;
        self.instances = self.store.load()?;
        Ok(())
    }

    pub fn catalog(&self) -> Vec<ToolCatalogItem> {
        self.registry.catalog()
    }

    pub fn list(&self) -> Vec<ToolInstance> {
        self.instances.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<ToolInstance> {
        self.instances.get(id).cloned()
    }

    pub fn create(&mut self, req: CreateToolInstanceRequest) -> anyhow::Result<ToolInstance> {
        let driver = self
            .registry
            .get(&req.kind)
            .ok_or_else(|| anyhow::anyhow!("unsupported tool kind"))?;
        let id = format!(
            "tool_{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        );
        let config = driver.default_config();
        driver.validate(&config)?;

        let instance = ToolInstance {
            id: id.clone(),
            kind: req.kind,
            name: req
                .name
                .unwrap_or_else(|| format!("{} instance", driver.display_name())),
            enabled: true,
            version: 1,
            config,
        };
        self.instances.insert(id, instance.clone());
        self.store.save(&self.instances)?;
        Ok(instance)
    }

    pub fn update(&mut self, id: &str, req: UpdateToolInstanceRequest) -> anyhow::Result<ToolInstance> {
        let existing = self
            .instances
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("tool not found"))?
            .clone();
        let driver = self
            .registry
            .get(&existing.kind)
            .ok_or_else(|| anyhow::anyhow!("driver not found"))?;
        driver.validate(&req.config)?;

        let updated = ToolInstance {
            id: existing.id,
            kind: existing.kind,
            name: req.name,
            enabled: req.enabled,
            version: existing.version + 1,
            config: req.config,
        };
        self.instances.insert(id.to_string(), updated.clone());
        self.store.save(&self.instances)?;
        Ok(updated)
    }

    /// Picks the first enabled tool instance, preferring Claude Code, then other registered kinds.
    pub fn pick_enabled_chat_driver(&self) -> Option<(ToolInstance, Arc<dyn CodingToolDriver>)> {
        let priority = [
            ToolKind::ClaudeCode,
            ToolKind::CursorCli,
            ToolKind::Codex,
            ToolKind::QwenCode,
            ToolKind::QoderCli,
            ToolKind::KimiCli,
        ];
        for kind in priority {
            for inst in self.instances.values() {
                if inst.enabled && inst.kind == kind {
                    if let Some(driver) = self.registry.get(&kind) {
                        return Some((inst.clone(), driver));
                    }
                }
            }
        }
        None
    }

    /// Runs a cancellable code-chat turn tracked by `task_id` in the runtime registry.
    pub fn execute_code_chat_for_task(
        &self,
        workspace: &Path,
        messages: &[ToolChatMessage],
        task_id: &str,
        runtime: &crate::tasks::TaskRuntimeRegistry,
    ) -> anyhow::Result<(ToolInstance, ToolExecutionResult)> {
        let (instance, driver) = self
            .pick_enabled_chat_driver()
            .ok_or_else(|| anyhow::anyhow!("no_enabled_coding_tool"))?;
        let spec = driver.chat_subprocess_spec(&instance.config, messages)?;
        let (stdout, stderr, exit_code) =
            runtime.run_chat_subprocess_cancellable(&spec, Some(workspace), task_id)?;
        let merged = crate::tools::driver::merge_shell_output(&stdout, &stderr);
        let usage = driver.collect_usage(&instance.config, messages, &merged)?;
        Ok((
            instance,
            ToolExecutionResult {
                output: merged,
                exit_code,
                usage,
            },
        ))
    }

    /// Streams stdout chunks through `delta_tx` while the tool runs.
    pub async fn execute_code_chat_streaming(
        &self,
        workspace: &Path,
        messages: &[ToolChatMessage],
        delta_tx: tokio::sync::mpsc::Sender<String>,
    ) -> anyhow::Result<(ToolInstance, ToolExecutionResult)> {
        let (instance, driver) = self
            .pick_enabled_chat_driver()
            .ok_or_else(|| anyhow::anyhow!("no_enabled_coding_tool"))?;
        let session: ToolSession = driver.create_session(&instance.config)?;
        let _ = session;
        let spec = driver.chat_subprocess_spec(&instance.config, messages)?;
        let (merged, exit_code) = chat_stream::stream_chat_subprocess(&spec, workspace, delta_tx).await?;
        let usage = driver.collect_usage(&instance.config, messages, &merged)?;
        Ok((
            instance,
            ToolExecutionResult {
                output: merged,
                exit_code,
                usage,
            },
        ))
    }
}
