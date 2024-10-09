use std::{
    collections::{HashMap, HashSet},
    f64::consts::PI,
};

use crate::{
    layer::{SelectMany, SelectOne},
    molecule_layer::MoleculeLayer,
    n_to_n::NtoN,
};
use nalgebra::{Isometry3, Translation3, Unit, UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Substituent {
    direction: SelectOne,
    on_body: SelectOne,
    structure: MoleculeLayer,
    pub substituent_name: String,
}

#[derive(Debug)]
#[allow(dead_code, reason = "only use for error output")]
pub enum SubstituentError {
    EntryAtomNotFoundInTarget(SelectOne),
    ExitAtomNotFoundInTarget(SelectOne),
    DirectionAtomNotFoundInSubstituent(SelectOne),
    OnBodyAtomNotFoundInSusbstituent(SelectOne),
}

impl Substituent {
    pub fn new(
        direction: SelectOne,
        on_body: SelectOne,
        structure: MoleculeLayer,
        substituent_name: String,
    ) -> Self {
        Self {
            direction,
            on_body,
            structure,
            substituent_name,
        }
    }

    pub fn generate_layer(
        &self,
        target: &MoleculeLayer,
        entry: SelectOne,
        exit: SelectOne,
    ) -> Result<MoleculeLayer, SubstituentError> {
        let target_entry = entry
            .get_atom(&target)
            .ok_or(SubstituentError::EntryAtomNotFoundInTarget(entry.clone()))?;
        let target_exit = exit
            .get_atom(&target)
            .ok_or(SubstituentError::ExitAtomNotFoundInTarget(exit.clone()))?;
        let a = target_exit.position - target_entry.position;
        let substituent_direction = self.direction.get_atom(&self.structure).ok_or(
            SubstituentError::DirectionAtomNotFoundInSubstituent(self.direction.clone()),
        )?;
        let substituent_on_body = self.on_body.get_atom(&self.structure).ok_or(
            SubstituentError::OnBodyAtomNotFoundInSusbstituent(self.on_body.clone()),
        )?;
        let b = substituent_direction.position - substituent_on_body.position;
        let axis = b.cross(&a);
        let axis = Unit::new_normalize(if axis.norm() == 0. {
            Vector3::x()
        } else {
            axis
        });
        let angle = (b.dot(&a) / (a.norm() * b.norm())).acos();
        let angle = if angle.is_nan() { PI } else { angle };
        let translation = Translation3::from(target_exit.position - substituent_direction.position);
        let rotation = UnitQuaternion::new(angle * *axis);
        let rotation = Isometry3::from_parts(Translation3::from(Vector3::zeros()), rotation);
        let mut substituent = self.structure.clone();
        let select = SelectMany::All.to_indexes(&substituent);
        let pre_translation = Translation3::from(-substituent_direction.position);
        let post_translation = pre_translation.inverse();
        substituent.atoms.isometry(pre_translation.into(), &select);
        substituent.atoms.isometry(rotation, &select);
        substituent.atoms.isometry(post_translation.into(), &select);
        substituent.atoms.isometry(translation.into(), &select);
        let replace_atom = self
            .on_body
            .get_atom(&substituent)
            .expect("unable to get exit atom in substituent");
        self.direction.set_atom(&mut substituent, None);
        self.on_body.set_atom(&mut substituent, None);
        let offset = target.atoms.len();
        let mut substituent = substituent.offset(offset);
        // set on_body atom on entry position.
        let entry_index = entry
            .to_index(target)
            .expect("here will never return None as the atom got uppon");
        substituent
            .atoms
            .set_atoms(entry_index, vec![Some(replace_atom)]);
        let neighbors = substituent
            .bonds
            .get_neighbors(
                self.on_body
                    .to_index(&substituent)
                    .expect("Index should be able to get here.") + offset,
            )
            .expect("Neighbors should be able to get here.")
            .cloned()
            .collect::<Vec<_>>();
        for (index, bond) in neighbors.into_iter().enumerate() {
            if bond.is_some() {
                println!("{}", index);
            }
            substituent.bonds.set_bond(entry_index, index, bond);
        }
        substituent.groups = NtoN::from(
            substituent
                .groups
                .get_lefts()
                .into_iter()
                .map(|current_name| {
                    let updated_name = self.substituent_name.clone();
                    let updated_name = [updated_name, current_name.to_string()].join("_");
                    substituent
                        .groups
                        .get_left(current_name)
                        .map(move |index| (updated_name.clone(), *index))
                })
                .flatten()
                .collect::<HashSet<_>>(),
        );
        substituent.ids = HashMap::new();
        substituent.title = [target.title.to_string(), self.substituent_name.to_string()].join("_");
        Ok(substituent)
    }
}
