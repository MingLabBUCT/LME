use std::{collections::BTreeMap, fs::File, path::PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{
    runner::{Runner, RunnerOutput},
    workflow_data::WorkflowData,
};

#[derive(Debug, Deserialize)]
pub struct Step {
    from: Option<String>,
    name: Option<String>,
    run: Runner,
}

impl Step {
    pub fn execute(self, index: usize, workflow_data: &mut WorkflowData) -> Result<()> {
        if let Some(from) = self.from {
            let window = workflow_data
                .windows
                .get(&from)
                .cloned()
                .with_context(|| format!("Failed to load window with name {}", from))?;
            workflow_data.current_window = window;
        }
        let generated_stacks = self.run.execute(
            &workflow_data.base,
            &workflow_data.current_window,
            &mut workflow_data.layers.borrow_mut(),
        )?;
        match generated_stacks {
            RunnerOutput::SingleWindow(generated_stacks) => {
                workflow_data.current_window = generated_stacks;
            }
            RunnerOutput::MultiWindow(named_stacks) => {
                let prefix = self.name.clone().unwrap_or(index.to_string());
                workflow_data.current_window = BTreeMap::new();
                for (suffix, generated_stacks) in named_stacks {
                    workflow_data
                        .current_window
                        .extend(generated_stacks.clone());
                    let name = [prefix.to_string(), suffix].join("_");
                    workflow_data.windows.insert(name, generated_stacks);
                }
            }
            RunnerOutput::None => {}
        };
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

#[derive(Debug, Deserialize, Default)]
#[serde(try_from = "StepsLoader")]
pub struct Steps(pub Vec<Step>);

#[derive(Deserialize, Debug)]
struct StepsLoader(Vec<StepLoader>);

impl TryFrom<StepsLoader> for Steps {
    type Error = anyhow::Error;

    fn try_from(value: StepsLoader) -> Result<Self> {
        let mut inner = vec![];
        for loader in value.0 {
            let Steps(result) = Steps::try_from(loader)?;
            inner.extend(result);
        }
        Ok(Steps(inner))
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum StepLoader {
    Data(Step),
    Loader(PathBuf),
}

impl TryFrom<StepLoader> for Steps {
    type Error = anyhow::Error;
    fn try_from(value: StepLoader) -> Result<Self> {
        match value {
            StepLoader::Data(step) => Ok(Steps(vec![step])),
            StepLoader::Loader(filepath) => {
                let current_directory = std::env::current_dir().with_context(|| {
                    format!(
                        "Unable to get current working directory for loading {:?}",
                        filepath
                    )
                })?;
                println!(
                    "Loading {:?} from working directory {:?}",
                    filepath, current_directory
                );
                let load_file_parent = filepath.parent().with_context(|| {
                    format!(
                        "Failed to get parent directory of {:?}, workding direcotry is {:?}",
                        filepath, current_directory
                    )
                })?;
                let file = File::open(&filepath).with_context(|| {
                    format!(
                        "Failed to open target file {:?} in working directory {:?}",
                        filepath, current_directory
                    )
                })?;
                std::env::set_current_dir(load_file_parent).with_context(|| {
                    format!("Failed to set {:?} as working directory", load_file_parent)
                })?;
                let result = serde_yaml::from_reader(file)?;
                std::env::set_current_dir(&current_directory).with_context(|| {
                    format!(
                        "Failed to set working directory back to {:?}",
                        current_directory
                    )
                })?;
                Ok(result)
            }
        }
    }
}
