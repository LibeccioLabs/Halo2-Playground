use halo2_proofs::{
    circuit::{Chip, Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Selector},
    poly::Rotation,
};

use crate::utilities::RegionSequenceAssignment;

use super::Number;

mod chip_setup_api;
mod gate_implementation;

pub(crate) struct SudokuProblemAssignment<const SIZE: usize, F: ff::Field> {
    pub problem_grid: [[Number<F>; SIZE]; SIZE],
    pub solution_grid: [[Number<F>; SIZE]; SIZE],
}

#[derive(Debug, Clone)]
pub(crate) struct SudokuProblemChip<const SIZE: usize, F: ff::Field> {
    config: SPConfig<SIZE>,
    _marker: std::marker::PhantomData<F>,
}

#[derive(Debug, Clone)]
pub(crate) struct SPConfig<const SIZE: usize> {
    pub grid_columns: [Column<Advice>; SIZE],

    s_grid_compatibility: Selector,
}

impl<const SIZE: usize, F: ff::Field> halo2_proofs::circuit::Chip<F>
    for SudokuProblemChip<SIZE, F>
{
    type Config = SPConfig<SIZE>;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }
    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}
