use crate::{utilities::DivModCounter, Number};

use std::cell::RefCell;

use halo2_proofs::{
    circuit::{Layouter, Region, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Selector},
    poly::Rotation,
};

use try_collect::ForceCollect;

/// in this module, we implement the functions needed to setup
/// the advice columns in the right way to enforce the permutation constraint.
mod chip_setup_api;
/// in this module, we implement the gate logic.
mod gate_implementation;

#[derive(Debug, Clone)]
pub(crate) struct PermutationChip<const N_OBJECTS: usize, F: ff::Field> {
    config: PConfig<N_OBJECTS>,
    _marker: std::marker::PhantomData<F>,
}

#[derive(Debug, Clone)]
pub(crate) struct PConfig<const N_OBJECTS: usize> {
    pub item_columns: [Column<Advice>; N_OBJECTS],

    swap_selector_columns: Vec<Column<Advice>>,

    /// This field states where, relative to the start of the gate region,
    /// the permutated items are located.
    #[allow(dead_code)]
    output_item_positions: [(Column<Advice>, Rotation); N_OBJECTS],

    s_perm: Selector,
}

impl<const N_OBJECTS: usize> PConfig<N_OBJECTS> {
    pub fn get_item_columns(&self) -> &[Column<Advice>; N_OBJECTS] {
        &self.item_columns
    }

    #[allow(dead_code)]
    pub fn get_output_item_relative_positions(&self) -> &[(Column<Advice>, Rotation); N_OBJECTS] {
        &self.output_item_positions
    }
}

impl<const N_OBJECTS: usize, F: ff::Field> halo2_proofs::circuit::Chip<F>
    for PermutationChip<N_OBJECTS, F>
{
    type Config = PConfig<N_OBJECTS>;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }
    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

/// A sequence of swaps that corresponds to the swaps attempted by bubble sort.
fn bubble_sort_swap_schedule<const N_OBJECTS: usize>() -> Vec<(usize, usize)> {
    // not efficient but this at least keeps the implementations coherent
    from_permutation_to_bubble_sort_swap_schedule::<N_OBJECTS>(
        (0..N_OBJECTS).f_collect("number of items is correct"),
    )
    .into_iter()
    .map(|(_, i1, i2)| (i1, i2))
    .collect()
}

/// Given a permutation of `N_OBJECTS` objects it outputs a vector consisting
/// of `(bool, usize, usize)` triplets
fn from_permutation_to_bubble_sort_swap_schedule<const N_OBJECTS: usize>(
    mut permutation: [usize; N_OBJECTS],
) -> Vec<(bool, usize, usize)> {
    if N_OBJECTS < 2 {
        return vec![];
    }
    let mut bubble_sort_schedule = Vec::with_capacity(N_OBJECTS * (N_OBJECTS - 1) / 2);
    for i in (0..N_OBJECTS).rev() {
        for j in 0..i {
            bubble_sort_schedule.push((
                {
                    if permutation[j] > permutation[j + 1] {
                        permutation.swap(j, j + 1);
                        true
                    } else {
                        false
                    }
                },
                j,
                j + 1,
            ));
        }
    }

    bubble_sort_schedule
}
