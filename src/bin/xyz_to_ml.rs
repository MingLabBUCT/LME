use std::fs::File;

use clap::Parser;
use glob::glob;
use lmers::io::BasicIOMolecule;
use lmers::sparse_molecule::{SparseAtomList, SparseBondMatrix, SparseMolecule};

#[derive(Parser)]
#[command(version, about, long_about = None)]
/// Convert XYZ files to SparseMolecule data in JSON(.ml.json) or YAML(.ml.yaml) format.
///
/// If neither -j/--json nor -y/--yaml is set, nothing will be output but check the XYZ files could be convert.
struct Arguments {
    /// Give the global file match pattern, for example:
    ///
    /// - "./*.xyz" matches all xyz files in current working directory
    ///
    /// - "./abc-*.xyz" matches all xyz files starts with abc- in current working directory
    ///
    /// - "./**/*.xyz" matches all xyz files can be found recursively in current working directory
    #[arg(short, long)]
    input: String,
    /// Generate output SparseMolecule file in JSON format.
    #[arg(short, long)]
    json: bool,
    /// Generate output SparseMolecule file in YAML format.
    #[arg(short, long)]
    yaml: bool,
}

fn main() {
    let arg = Arguments::parse();
    let matched_paths = glob(&arg.input).unwrap();
    for path in matched_paths {
        let path = path.unwrap();
        let content = {
            println!("Read file {:#?}", path);
            let file = File::open(&path).unwrap();
            let structure = BasicIOMolecule::input("xyz", file).unwrap();
            let bonds = SparseBondMatrix::new(structure.atoms.len());
            let atoms = SparseAtomList::from(structure.atoms);
            SparseMolecule {
                atoms,
                bonds,
                ids: None,
                groups: None,
            }
        };

        if arg.json {
            let mut ml_path = path.clone();
            ml_path.set_extension("ml.json");
            let ml_file = File::create(ml_path).unwrap();
            serde_json::to_writer(ml_file, &content).unwrap();
        }

        if arg.yaml {
            let mut ml_path = path.clone();
            ml_path.set_extension("ml.yaml");
            let ml_file = File::create(ml_path).unwrap();
            serde_yaml::to_writer(ml_file, &content).unwrap();
        }
    }
}
