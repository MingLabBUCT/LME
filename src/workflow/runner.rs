use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use cached::{proc_macro::cached, UnboundCache};
use lme::chemistry::element_num_to_symbol;
use lme::io::AtomListMap;
use lme::layer::{Layer, SelectOne};
use lme::molecule_layer::{Atom3D, MoleculeLayer};
use lme::serde_default::default_xyz;
use lme::substituent::{Substituent, SubstituentError};
use lme::workspace::{LayerStorage, LayerStorageError};
use serde::Deserialize;
use tempfile::tempdir;

use crate::error::WorkflowError;

use glob::glob;
use rayon::prelude::*;

#[derive(Debug, Deserialize)]
pub enum Runner {
    AddLayers(Vec<Layer>),
    Substituent {
        entry: SelectOne,
        exit: SelectOne,
        file_pattern: String,
    },
    Function {
        command: String,
        arguments: Vec<String>,
    },
    OutputXYZ {
        prefix: String,
        suffix: String,
        path_prefix: String,
        #[serde(default = "default_xyz")]
        extension: String,
    },
}

#[derive(Deserialize)]
pub enum RunnerOutput {
    Serial(Vec<Vec<usize>>),
    Named(BTreeMap<String, Vec<Vec<usize>>>),
    None,
}

impl Runner {
    pub fn execute<'a>(
        self,
        base: &MoleculeLayer,
        current_window: Vec<&Vec<usize>>,
        layer_storage: &mut LayerStorage,
    ) -> Result<RunnerOutput, WorkflowError> {
        match self {
            Self::AddLayers(layers) => {
                let layer_ids = layer_storage.create_layers(layers);
                Ok(RunnerOutput::Serial(
                    current_window
                        .into_iter()
                        .map(|current| {
                            let mut current = current.clone();
                            current.extend(layer_ids.clone());
                            current
                        })
                        .collect(),
                ))
            }
            Self::Function { command, arguments } => {
                let input = current_window
                    .into_par_iter()
                    .map(|stack_path| cached_read_stack(base, &layer_storage, &stack_path))
                    .collect::<Result<Vec<_>, _>>()?;
                let input = serde_json::to_string(&input)
                    .map_err(|err| WorkflowError::SerdeJSONError(err))?;
                let temp_directory =
                    tempdir().map_err(|err| WorkflowError::TempDirCreateError(err))?;
                let filepath = temp_directory.path().join("stacks.json");
                let mut file = File::create(&filepath)
                    .map_err(|err| WorkflowError::FileWriteError((filepath.clone(), err)))?;
                file.write_all(input.as_bytes())
                    .map_err(|err| WorkflowError::FileWriteError((filepath, err)))?;
                let exit_status = Command::new(&command)
                    .args(&arguments)
                    .current_dir(&temp_directory)
                    .status()
                    .map_err(|err| {
                        WorkflowError::CommandExecutionFail((
                            command.to_string(),
                            arguments.clone(),
                            err,
                        ))
                    })?;
                if !exit_status.success() {
                    Err(WorkflowError::CommandExitStatus(exit_status))?;
                }
                let filepath = temp_directory.path().join("output.json");
                let file = File::open(&filepath)
                    .map_err(|err| WorkflowError::FileReadError((filepath, err)))?;
                let output: RunnerOutput = serde_json::from_reader(file)?;
                Ok(output)
            }
            Self::Substituent {
                entry,
                exit: target,
                file_pattern,
            } => {
                let matched_files = glob(&file_pattern)?.collect::<Result<Vec<_>, _>>()?;
                let matched_files = matched_files
                    .into_par_iter()
                    .map(|path| {
                        File::open(&path).map_err(|err| WorkflowError::FileReadError((path, err)))
                    })
                    .collect::<Result<Vec<_>, WorkflowError>>()?;
                let substituents = matched_files
                    .into_par_iter()
                    .map(|file| serde_yaml::from_reader(file))
                    .collect::<Result<Vec<Substituent>, serde_yaml::Error>>()?;
                let current_structures = current_window
                    .iter()
                    .map(|stack_path| cached_read_stack(base, &layer_storage, &stack_path))
                    .collect::<Result<Vec<_>, LayerStorageError>>()?;
                let mut result = BTreeMap::new();
                for substituent in substituents {
                    let new_layers = current_structures
                        .par_iter()
                        .map(|base| substituent.generate_layer(base, entry.clone(), target.clone()))
                        .collect::<Result<Vec<_>, SubstituentError>>()?;
                    let layer_ids = layer_storage
                        .create_layers(new_layers.into_iter().map(|ml| Layer::Fill(ml)));
                    let new_stacks = layer_ids
                        .enumerate()
                        .map(|(index, layer_id)| {
                            let mut stack_path = current_window[index].clone();
                            stack_path.push(layer_id);
                            stack_path
                        })
                        .collect::<Vec<_>>();
                    result.insert(substituent.substituent_name, new_stacks);
                }
                Ok(RunnerOutput::Named(result))
            }
            Runner::OutputXYZ {
                prefix,
                suffix,
                path_prefix: filename_pattern,
                extension,
            } => {
                let outputs = current_window
                    .into_par_iter()
                    .map(|stack_path| {
                        let data = cached_read_stack(base, &layer_storage, &stack_path)?;
                        let atom_map = AtomListMap::from(&data.atoms);
                        let xyz = data
                            .atoms
                            .data()
                            .iter()
                            .filter_map(|atom| {
                                atom.and_then(|Atom3D { element, position }| {
                                    element_num_to_symbol(&element).map(|element| {
                                        format!(
                                            "{} {} {} {}",
                                            element, position.x, position.y, position.z
                                        )
                                    })
                                })
                            })
                            .collect::<Vec<_>>();
                        Ok((data.title, atom_map, xyz))
                    })
                    .collect::<Result<Vec<_>, LayerStorageError>>()?;
                let path = PathBuf::from(&filename_pattern);
                for (title, atom_map, xyz) in outputs {
                    let path = path.clone();
                    let mut path = path.join(&title);
                    let atoms_len = xyz.len().to_string();
                    let content = [
                        vec![prefix.clone(), atoms_len, title],
                        xyz,
                        vec![suffix.clone()],
                    ]
                    .concat()
                    .into_iter()
                    .skip_while(|line| line == "")
                    .collect::<Vec<_>>()
                    .join("\n");
                    path.set_extension(&extension);
                    File::create_new(&path)
                        .map_err(|err| WorkflowError::FileWriteError((path.clone(), err)))?
                        .write_all(content.as_bytes())
                        .map_err(|err| WorkflowError::FileWriteError((path.clone(), err)))?;
                    path.set_extension("atommap.json");
                    let atom_map_file = File::create_new(&path)
                        .map_err(|err| WorkflowError::FileWriteError((path.clone(), err)))?;
                    serde_json::to_writer(atom_map_file, &atom_map)?;
                }
                Ok(RunnerOutput::None)
            }
        }
    }
}

/// In a workflow, the base and existed layers will not be modified or deleted,
/// so the result of read_stack function is in fact only dependent on the path
/// parameter so create a cached function here is reasonable.
///
/// The read_stack function may return an Err(LayerStorageError), which
/// means there might be something wrong in program or input file, and the workflow
/// will exit, so the cache of error result will never be accessed in practice.
#[cached(
    ty = "UnboundCache<String, Result<MoleculeLayer, LayerStorageError>>",
    create = "{ UnboundCache::new() }",
    convert = r#"{ stack_path.iter().map(|item| item.to_string()).collect::<Vec<_>>().join("/") }"#
)]
fn cached_read_stack(
    base: &MoleculeLayer,
    layer_storage: &LayerStorage,
    stack_path: &[usize],
) -> Result<MoleculeLayer, LayerStorageError> {
    if let Some((last, heads)) = stack_path.split_last() {
        let layer = layer_storage
            .read_layer(last)
            .ok_or(LayerStorageError::NoSuchLayer(*last))?;
        let lower_result = cached_read_stack(base, layer_storage, heads)?;
        layer
            .filter(lower_result)
            .map_err(|err| LayerStorageError::FilterError(err))
    } else {
        Ok(base.clone())
    }
}
