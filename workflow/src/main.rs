use std::{
    fs::File,
    io::{Read, Write},
};

use anyhow::Context;
use input_data::{WorkflowCheckPoint, WorkflowInput};
use workflow_data::WorkflowData;

mod input_data;
mod io;
mod runner;
mod step;
mod workflow_data;
mod workspace;

fn main() {
    let input: WorkflowInput = serde_yaml::from_reader(
        File::open("lme_workflow.inp.yaml")
            .with_context(|| "Failed to open lme_workflow.inp.yaml in current directory")
            .unwrap(),
    )
    .unwrap();

    let checkpoint = load_checkpoint();
    let (skip, mut workflow_data) = if let Some(checkpoint) = checkpoint {
        (checkpoint.skip, checkpoint.workflow_data)
    } else {
        (0, WorkflowData::new(input.base))
    };

    let mut checkpoint = WorkflowCheckPoint {
        skip,
        workflow_data: workflow_data.clone(),
    };

    for (index, step) in input.steps.into_iter().enumerate().skip(skip) {
        println!(
            "Enter step: {}, window size: {}",
            index,
            workflow_data.current_window.len()
        );
        match step.execute(index, &mut workflow_data) {
            Ok(_) => {
                if !input.no_checkpoint {
                    checkpoint = WorkflowCheckPoint {
                        skip: index + 1,
                        workflow_data: workflow_data.clone(),
                    };
                }
            }
            Err(err) => {
                if !input.no_checkpoint {
                    println!("Error. Saving checkpoint file");
                    dump_checkpoint(&checkpoint);
                }
                panic!("{:#?}", err)
            }
        }
    }

    if !input.no_checkpoint {
        println!("Finished. Saving checkpoint file");
        dump_checkpoint(&checkpoint);
    }

    println!("finished");
}

fn load_checkpoint() -> Option<WorkflowCheckPoint> {
    let mut skip_file = File::open("lme_workflow.chk.skip").ok()?;
    let mut skip: String = String::new();
    skip_file
        .read_to_string(&mut skip)
        .with_context(|| "Failed to read lme_workflow.chk.skip thought the file exist")
        .unwrap();
    let skip: usize = skip
        .parse()
        .with_context(|| {
            format!(
                "Unable to parse skip steps in lme_workflow.chk.skip, content in file is: {}",
                skip
            )
        })
        .unwrap();
    let workflow_data_file = File::open("lme_workflow.chk.data")
        .with_context(|| "lme_workflow.chk.skip existed but lme_workflow.chk.data not found")
        .unwrap();
    let workflow_data: WorkflowData = serde_yaml::from_reader(
        zstd::Decoder::new(workflow_data_file)
            .with_context(|| "Failed to create zstd decompress pipe")
            .unwrap(),
    )
    .with_context(|| "Unable to deserialize lme_workflow.chk.data, it might be broken")
    .unwrap();
    Some(WorkflowCheckPoint {
        skip,
        workflow_data,
    })
}

fn dump_checkpoint(checkpoint: &WorkflowCheckPoint) {
    File::create("lme_workflow.chk.skip")
        .with_context(|| "Unable to create lme_workflow.chk.skip")
        .unwrap()
        .write_all(checkpoint.skip.to_string().as_bytes())
        .with_context(|| "Unable to write to lme_workflow.chk.skip")
        .unwrap();
    serde_yaml::to_writer(
        zstd::Encoder::new(
            File::create("lme_workflow.chk.data")
                .with_context(|| "Unable to create lme_workflow.chk.data")
                .unwrap(),
            9,
        )
        .with_context(|| "Unable to create zstd compress pipe")
        .unwrap().auto_finish(),
        &checkpoint.workflow_data,
    )
    .with_context(|| "Unable to serialize lme_workflow.chk.data")
    .unwrap()
}
