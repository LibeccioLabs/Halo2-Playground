/// This chip implements a gate that enforces two
/// sets of values to be a permutation of each other.
pub(crate) mod permutation_chip;

mod permutation_circuit;
pub use permutation_circuit::PermutationCircuit;

mod sudoku_circuit;
pub use sudoku_circuit::SudokuCircuit;

/// This chip implements a gate that enforces two
/// grids to be a couple of compatible problem-solution
/// sudoku grids.
mod sudoku_problem_chip;

/// A variable representing a number.
#[derive(Clone)]
pub struct Number<F: ff::Field>(halo2_proofs::circuit::AssignedCell<F, F>);

/// This module defines a utility trait that allows to easily assign arrays and grids of values to columns.
mod region_sequence_assignment;
