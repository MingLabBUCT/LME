use std::{collections::BTreeMap, env::current_dir, fs::File, io::Read};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use url::Url;

use super::{
    runner::{Runner, RunnerOutput},
    workflow_data::WorkflowData,
};

#[derive(Debug, Deserialize)]
pub struct Step {
    pub from: Option<String>,
    pub name: Option<String>,
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
            &workflow_data.layers,
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
struct StepLoader {
    from: Option<String>,
    name: Option<String>,
    run: Option<Runner>,
    load: Option<String>,
}

impl TryFrom<StepLoader> for Steps {
    type Error = anyhow::Error;
    fn try_from(value: StepLoader) -> Result<Self> {
        if let Some(filepath) = value.load {
            let url = if filepath.starts_with("/") {
                Url::parse(&format!("file:{}", filepath))?
            } else {
                let url = Url::from_directory_path(current_dir()?)
                    .map_err(|_| anyhow!("Unable to get current working direcotry"))?;
                url.join(&filepath)?
            };
            let filepath = url
                .to_file_path()
                .map_err(|_| anyhow!("Unable to convert URL {} to filepath", url))?;
            if filepath
                .file_stem()
                .with_context(|| anyhow!("Filename with no file stem is not allowed now"))?
                .to_string_lossy()
                .to_string()
                .ends_with("template")
            {
                println!("Loading template {:?} with query string: {:?}", filepath, url.query());
                let mut file = File::open(&filepath)
                    .with_context(|| format!("Failed to open target file {:?}", filepath))?;
                let mut content = String::new();
                file.read_to_string(&mut content)
                    .with_context(|| anyhow!("Failed to read file {:?}", &filepath))?;
                for (k, v) in url.query_pairs() {
                    let k = format!("{{{{ {} }}}}", k);
                    content = content.replace(&k, &v);
                }
                Ok(serde_yaml::from_str(&content)?)
            } else {
                println!("Loading {:?}", filepath);
                let file = File::open(&filepath)
                    .with_context(|| format!("Failed to open target file {:?}", filepath))?;
                Ok(serde_yaml::from_reader(file)?)
            }
        } else if let Some(runner) = value.run {
            Ok(Steps(vec![Step {
                from: value.from,
                name: value.name,
                run: runner,
            }]))
        } else {
            Err(anyhow!(format!(
                "No load or run field is specified in {:#?}",
                value
            )))
        }
    }
}
