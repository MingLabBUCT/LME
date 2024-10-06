use serde::Deserialize;

use crate::{
    error::WorkflowError,
    runner::{Runner, RunnerOutput},
    workflow_data::WorkflowData,
};

#[derive(Deserialize)]
pub struct Step {
    from: Option<String>,
    name: Option<String>,
    run: Runner,
}

impl Step {
    pub fn execute(
        self,
        index: usize,
        workflow_data: &mut WorkflowData,
    ) -> Result<(), WorkflowError> {
        if let Some(from) = self.from {
            let window = workflow_data
                .windows
                .get(&from)
                .cloned()
                .ok_or(WorkflowError::WindowNotFound(from.clone()))?;
            workflow_data.current_window = window;
        }
        let current_window = workflow_data.current_window_stacks()?;
        let generated_stacks = self.run.execute(
            &workflow_data.base,
            current_window,
            &mut workflow_data.layers.borrow_mut(),
        )?;
        let start = workflow_data.stacks.len();
        match generated_stacks {
            RunnerOutput::Serial(generated_stacks) => {
                workflow_data.stacks.extend(generated_stacks);
            }
            RunnerOutput::Named(named_stacks) => {
                let prefix = self.name.clone().unwrap_or(index.to_string());
                for (suffix, genenrated_stacks) in named_stacks {
                    let name = [prefix.to_string(), suffix].join("_");
                    let start = workflow_data.stacks.len();
                    workflow_data.stacks.extend(genenrated_stacks);
                    workflow_data
                        .windows
                        .insert(name, start..workflow_data.stacks.len());
                }
            }
            RunnerOutput::None => {}
        };
        workflow_data.current_window = start..workflow_data.stacks.len();
        if let Some(name) = self.name {
            if workflow_data
                .windows
                .insert(name.to_string(), workflow_data.current_window.clone())
                .is_some()
            {
                println!("Over take window named {}", name);
            }
        }
        Ok(())
    }
}
