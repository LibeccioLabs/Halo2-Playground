mod permutation_circuit;
pub use permutation_circuit::PermutationCircuit;

mod sudoku_circuit;
pub use sudoku_circuit::SudokuCircuit;

mod truncated_factorial_circuit;

/// This chip implements a gate that enforces two
/// sets of values to be a permutation of each other.
pub mod permutation_chip;

/// This chip implements a gate that enforces two
/// grids to be a couple of compatible problem-solution
/// sudoku grids.
pub mod sudoku_problem_chip;

/// This module implements a chip that, given an input number `N_FACTORS`,
/// given `F: ff::Field`, and
/// given an input number `n: F`, forces the output cell to be equal to
/// `n * (n + 1) * ... * (n + N_FACTORS - 1)`.
///
/// If the input `n` is equal to `F::ONE`, then the chip basically computes
/// the factorial of `N_FACTORS` modulo the field charachteristic of `F`.
pub mod truncated_factorial_chip;

/// General purpose functions and structs that are used by more than one chip,
/// or are otherwise not logically related to any particular chip.
pub mod utilities;

/// A variable representing a number.
#[derive(Clone)]
pub struct Number<F: ff::Field>(halo2_proofs::circuit::AssignedCell<F, F>);

impl<F: ff::Field> std::ops::Deref for Number<F> {
    type Target = halo2_proofs::circuit::AssignedCell<F, F>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
