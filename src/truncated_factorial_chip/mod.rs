use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{Chip, Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Selector},
    poly::Rotation,
};

use crate::{utilities::DivModCounter, Number};

mod chip_setup_api;
mod gate_implementation;

#[derive(Debug, Clone)]
pub struct TruncatedFactorialChip<
    F: ff::Field,
    const N_FACTORS: usize,
    const MUL_BATCH_SIZE: usize,
    const N_COLUMNS: usize,
> {
    config: TConfig<N_COLUMNS>,
    _marker: PhantomData<F>,
}

#[derive(Debug, Clone)]
pub struct TConfig<const N_COLUMNS: usize> {
    pub columns: [Column<Advice>; N_COLUMNS],
    s_fact: Selector,
}

impl<F: ff::Field, const N_FACTORS: usize, const MUL_BATCH_SIZE: usize, const N_COLUMNS: usize>
    halo2_proofs::circuit::Chip<F>
    for TruncatedFactorialChip<F, N_FACTORS, MUL_BATCH_SIZE, N_COLUMNS>
{
    type Config = TConfig<N_COLUMNS>;
    type Loaded = ();
    fn config(&self) -> &Self::Config {
        &self.config
    }
    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}
