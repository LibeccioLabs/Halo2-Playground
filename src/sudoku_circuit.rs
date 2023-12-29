use crate::{
    permutation_chip::PermutationChip, sudoku_problem_chip::SudokuProblemChip,
    utilities::RegionSequenceAssignment,
};
use halo2_proofs::{
    circuit::Value,
    circuit::{Layouter, SimpleFloorPlanner},
    plonk::{Column, ConstraintSystem, Error, Fixed, Instance},
};
use std::collections::{BTreeMap, BTreeSet};
use try_collect::ForceCollect;

/// A circuit that proves that the input and output values are a permutation of one another.
#[derive(Clone, Debug)]
pub struct SudokuCircuit<F: ff::Field, const SIZE: usize, const SIZE_SQRT: usize> {
    problem: Value<[[F; SIZE]; SIZE]>,
    solution: Value<[[F; SIZE]; SIZE]>,
    symbols: [F; SIZE],
}

impl<F: ff::PrimeField, const SIZE: usize, const SIZE_SQRT: usize>
    SudokuCircuit<F, SIZE, SIZE_SQRT>
{
    pub fn new_unchecked(
        problem: Value<[[F; SIZE]; SIZE]>,
        solution: Value<[[F; SIZE]; SIZE]>,
        symbols: [F; SIZE],
    ) -> Self {
        Self {
            problem,
            solution,
            symbols,
        }
    }

    pub fn try_new(
        problem: [[F; SIZE]; SIZE],
        solution: [[F; SIZE]; SIZE],
        symbols: [F; SIZE],
    ) -> Result<Self, ()> {
        // We check that the compile time constants are consistent
        if SIZE != SIZE_SQRT * SIZE_SQRT {
            return Err(());
        }

        // We check that the provided symbols do not contain duplicates
        let mut duplicate_detector = BTreeSet::<Vec<u8>>::new();
        for s in symbols {
            let bytes = s.to_repr();
            if duplicate_detector.contains(bytes.as_ref()) {
                return Err(());
            }
            duplicate_detector.insert(Vec::from(bytes.as_ref()));
        }

        // We check that `F::ZERO` is not a symbol
        if duplicate_detector.contains(F::ZERO.to_repr().as_ref()) {
            return Err(());
        }

        // We check that the problem contains only symbols or `F::ZERO` entries
        if !problem.iter().all(|col| {
            col.into_iter()
                .all(|n| *n == F::ZERO || duplicate_detector.contains(n.to_repr().as_ref()))
        }) {
            return Err(());
        }

        // We check that the solution only contains symbols
        if !solution.iter().all(|col| {
            col.into_iter()
                .all(|n| duplicate_detector.contains(n.to_repr().as_ref()))
        }) {
            return Err(());
        }

        Ok(Self {
            problem: Value::known(problem),
            solution: Value::known(solution),
            symbols,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SudokuConfig<const SIZE: usize> {
    permutation_config: crate::permutation_chip::PConfig<SIZE>,
    grid_compatibility_config: crate::sudoku_problem_chip::SPConfig<SIZE>,
    public_problem_columns: [Column<Instance>; SIZE],
    sudoku_symbols_column: Column<Fixed>,
}

impl<F: ff::Field, const SIZE: usize, const SIZE_SQRT: usize> SudokuCircuit<F, SIZE, SIZE_SQRT> {
    /// Given a symbols array, outputs an instance of the circuit
    /// without witness values
    pub fn circuit_wiring_from_symbols(symbols: [F; SIZE]) -> Self {
        Self {
            problem: Value::unknown(),
            solution: Value::unknown(),
            symbols,
        }
    }
}

// This is the only implementation happening for
// F: ff::PrimeField instead of F: ff::Field.
// The (tiny) loss of generality is due to the fact that
// if we want to efficiently compute the witness for the sudoku,
// then we have to be able to put an order relationship
// among field elements, which is not possible with F: ff::Field
// but can be done by using the binary representation of F
// instances if F: ff::PrimeField.
impl<F: ff::PrimeField, const SIZE: usize, const SIZE_SQRT: usize> halo2_proofs::plonk::Circuit<F>
    for SudokuCircuit<F, SIZE, SIZE_SQRT>
{
    type Config = SudokuConfig<SIZE>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::circuit_wiring_from_symbols(self.symbols)
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        assert_eq!(SIZE_SQRT * SIZE_SQRT, SIZE);

        let public_problem_columns = [(); SIZE].map(|_| meta.instance_column());
        for col in public_problem_columns {
            meta.enable_equality(col);
        }

        let sudoku_symbols_column = meta.fixed_column();
        meta.enable_constant(sudoku_symbols_column);

        let item_columns = [(); SIZE].map(|_| meta.advice_column());
        let swap_selector_columns = (0..SIZE / 2).map(|_| meta.advice_column()).collect();

        SudokuConfig {
            permutation_config: PermutationChip::configure(
                meta,
                item_columns,
                swap_selector_columns,
            ),
            grid_compatibility_config: SudokuProblemChip::configure(meta, item_columns),
            public_problem_columns,
            sudoku_symbols_column,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let symbol_to_ordinal = BTreeMap::from_iter(
            self.symbols
                .into_iter()
                .enumerate()
                .map(|(idx, sym)| (sym.to_repr().as_ref().to_owned(), idx)),
        );

        let grid_compatibility_chip =
            crate::sudoku_problem_chip::SudokuProblemChip::<SIZE, F>::construct(
                config.grid_compatibility_config,
            );

        let permutation_chip = crate::permutation_chip::PermutationChip::<SIZE, F>::construct(
            config.permutation_config,
        );

        // First thing, we have to declare the symbols that can go in a sudoku cell.
        // In practice, those will be encoded as the field element generated from 1 up to SIZE
        let symbol_cells = layouter.namespace(|| "symbols declaration").assign_region(
            || "symbols declaration",
            |mut region| {
                region.assign_array_to_column::<SIZE, _>(
                    config.sudoku_symbols_column,
                    0,
                    self.symbols.map(|x| Value::known(x)),
                )
            },
        )?;

        // Then, we allocate the problem and solution grids, making sure that
        // they are compatible (i.e. they describe the same sudoku problem).
        // We make sure they are compatible by feeding them
        // in the SudokuProblemChip chip.
        let crate::sudoku_problem_chip::SudokuProblemAssignment {
            problem_grid: problem_cells,
            solution_grid: solution_cells,
        } = grid_compatibility_chip.enforce_grid_compatibility(
            layouter.namespace(|| "sudoku problem setup and problem-solution compatibility"),
            self.problem
                .transpose_array()
                .map(|column| column.transpose_array()),
            self.solution
                .transpose_array()
                .map(|column| column.transpose_array()),
        )?;

        // We impose an equality constraint between the public output, and the `problem_cells`
        for (public_column, advice_column) in
            config.public_problem_columns.into_iter().zip(problem_cells)
        {
            for (row_idx, problem_cell) in advice_column.into_iter().map(|n| n.0.cell()).enumerate()
            {
                layouter.constrain_instance(problem_cell, public_column, row_idx)?;
            }
        }

        // from an `F` value we can build an `usize` value via the
        // symbol_to_ordinal map.
        // This way, we can obtain an array of usize from an array of F.
        // If the array contains all the symbols once, then
        // the array we obtain is a permutation. This permutation
        // is exactly the one needed to sort the symbols, and then
        // compare them with the values in the symbols column.
        let get_permutation =
            |input: [F; SIZE]| input.map(|x| symbol_to_ordinal[x.to_repr().as_ref()]);

        // We are going to apply a permutation to the cells of each of the solution's
        // rows, columns, and regions, to make each one of them equal to
        // symbols[0], ..., symbols[SIZE - 1]
        //
        // We are going to collect the output cells in this vectos, which we will later
        // use to enforce equality over them.
        let permutation_outputs = self
            .solution
            .zip(Value::known(Vec::with_capacity(3 * SIZE)))
            .map(|(solution, mut permutation_outputs)| {
                // For each column, we obtain its permutation that aligns it to the symbols
                for col_idx in 0..SIZE {
                    let col = solution[col_idx];
                    let alloc_col = solution_cells[col_idx].clone();

                    permutation_outputs.push(permutation_chip.apply_permutation(
                        layouter.namespace(|| "permutating column"),
                        alloc_col,
                        get_permutation(col),
                    ));
                }
                // We do the same for the rows
                for row_idx in 0..SIZE {
                    let row = solution.map(|col| col[row_idx]);
                    let alloc_row = (0..SIZE)
                        .map(|col_idx| solution_cells[col_idx][row_idx].clone())
                        .f_collect("the number of items is correct");

                    permutation_outputs.push(permutation_chip.apply_permutation(
                        layouter.namespace(|| "permutating row"),
                        alloc_row,
                        get_permutation(row),
                    ));
                }

                // And we do the same for the regions
                for region_col_offset in (0..SIZE_SQRT).map(|i| i * SIZE_SQRT) {
                    for region_row_offset in (0..SIZE_SQRT).map(|i| i * SIZE_SQRT) {
                        // An iterator over the grid positions that compose a sudoku region.
                        // for example, if SIZE == 4, SIZE_SQRT == 2,
                        // region_col_offset == 2, region_row_offset == 0,
                        // the iterator visits the cells marked in the image below,
                        // in the visualized order
                        // |-------|
                        // | | |0|2|
                        // |-------|
                        // | | |1|3|
                        // |-------|
                        // | | | | |
                        // |-------|
                        // | | | | |
                        // |-------|
                        let region_index_iter = (0..SIZE).map(|idx| {
                            (
                                region_col_offset + idx / SIZE_SQRT,
                                region_row_offset + idx % SIZE_SQRT,
                            )
                        });

                        let region = region_index_iter
                            .clone()
                            .map(|(col_idx, row_idx)| solution[col_idx][row_idx])
                            .f_collect("the number of items is correct");
                        let alloc_region = region_index_iter
                            .map(|(col_idx, row_idx)| solution_cells[col_idx][row_idx].clone())
                            .f_collect("the number of items is correct");

                        permutation_outputs.push(permutation_chip.apply_permutation(
                            layouter.namespace(|| "permutating region"),
                            alloc_region,
                            get_permutation(region),
                        ));
                    }
                }
                Result::<Vec<_>, _>::from_iter(permutation_outputs)
            });

        // If the result is known and is an error, we propagate the error.
        // This propagation method loses information about the error type,
        // but it is better than nothing.
        permutation_outputs.error_if_known_and(|result| result.is_err())?;
        // From now on we are sure that if `permutation_outputs` is known,
        // then it is not an error, and we can unwrap it.
        let permutation_outputs = permutation_outputs.map(
            |result|
            result.expect("if this was an error, the previous call to `error_if_known_and` would have returned an error.")
        );

        // Now we impose equality constraints among all permutation_outputs
        permutation_outputs
            .map(|permutation_outputs| {
                layouter
                    .namespace(|| "permutation equality constraints")
                    .assign_region(
                        || "permutation equality constraints",
                        |mut region| {
                            // For each permutation result, we constrain it to be equal to the loaded symbols.
                            for p_out in permutation_outputs.iter() {
                                for (left, right) in p_out.into_iter().zip(symbol_cells.iter()) {
                                    region.constrain_equal(left.cell(), right.cell())?;
                                }
                            }
                            Ok(())
                        },
                    )
            })
            // Same as before, the only way to unwrap an error from within a Value
            // seems to be this `error_if_known_and` hack.
            .error_if_known_and(|result| result.is_err())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use halo2_proofs::pasta::Fp;

    type SurokuGrid = [[Fp; 9]; 9];

    /// Helper function to generate symbols and a list of problems
    /// The return value is a tuple, laid out as
    /// `(symbols, impl Iterator<Item = (solution, problem)>)`
    fn setup_values(
        nr_random_masks_per_problem: usize,
    ) -> ([Fp; 9], impl IntoIterator<Item = (SurokuGrid, SurokuGrid)>) {
        let (symbols, grids_iter) = numeric_setup_values(nr_random_masks_per_problem);
        let symbols = symbols.map(|n| Fp::from(n as u64));

        let grids_iter = grids_iter.into_iter().map(move |(solution, problem)| {
            (
                solution.map(|col| col.map(|cell| symbols[cell - 1])),
                problem.map(|col| {
                    col.map(|cell| {
                        if cell == 0 {
                            Fp::from(0)
                        } else {
                            symbols[cell - 1]
                        }
                    })
                }),
            )
        });

        (symbols, grids_iter)
    }

    /// Helper function to generate symbols and a list of problems
    /// The return value is a tuple, laid out as
    /// `(symbols, impl Iterator<Item = (solution, problem)>)`
    ///
    /// The values provided are usize arrays. To use them in a
    /// sudoku circuit, they have to be converted in Fp values.
    fn numeric_setup_values(
        nr_random_masks_per_problem: usize,
    ) -> (
        [usize; 9],
        impl IntoIterator<Item = ([[usize; 9]; 9], [[usize; 9]; 9])>,
    ) {
        let symbols = core::array::from_fn(|n| n + 1);

        let grids = [
            [
                [2, 4, 9, 5, 3, 6, 1, 8, 7],
                [3, 5, 1, 2, 7, 8, 4, 9, 6],
                [6, 7, 8, 4, 9, 1, 5, 3, 2],
                [8, 9, 7, 1, 4, 5, 6, 2, 3],
                [4, 2, 3, 6, 8, 9, 7, 5, 1],
                [5, 1, 6, 7, 2, 3, 9, 4, 8],
                [1, 6, 2, 3, 5, 4, 8, 7, 9],
                [9, 3, 5, 8, 6, 7, 2, 1, 4],
                [7, 8, 4, 9, 1, 2, 3, 6, 5],
            ],
            [
                [1, 7, 5, 3, 4, 9, 8, 2, 6],
                [4, 2, 9, 8, 7, 6, 1, 5, 3],
                [3, 6, 8, 5, 1, 2, 9, 7, 4],
                [2, 8, 3, 7, 6, 1, 4, 9, 5],
                [7, 1, 6, 4, 9, 5, 2, 3, 8],
                [9, 5, 4, 2, 8, 3, 7, 6, 1],
                [8, 3, 7, 9, 5, 4, 6, 1, 2],
                [5, 9, 1, 6, 2, 8, 3, 4, 7],
                [6, 4, 2, 1, 3, 7, 5, 8, 9],
            ],
            [
                [1, 8, 5, 2, 7, 3, 6, 9, 4],
                [4, 2, 7, 8, 9, 6, 5, 3, 1],
                [3, 6, 9, 4, 1, 5, 7, 2, 8],
                [8, 1, 4, 3, 5, 2, 9, 7, 6],
                [6, 7, 3, 9, 4, 1, 8, 5, 2],
                [5, 9, 2, 7, 6, 8, 1, 4, 3],
                [9, 3, 1, 5, 8, 4, 2, 6, 7],
                [2, 5, 6, 1, 3, 7, 4, 8, 9],
                [7, 4, 8, 6, 2, 9, 3, 1, 5],
            ],
            [
                [1, 2, 3, 4, 5, 6, 7, 8, 9],
                [4, 5, 6, 7, 8, 9, 1, 2, 3],
                [7, 8, 9, 1, 2, 3, 4, 5, 6],
                [2, 1, 4, 3, 6, 5, 8, 9, 7],
                [3, 6, 5, 8, 9, 7, 2, 1, 4],
                [8, 9, 7, 2, 1, 4, 3, 6, 5],
                [5, 3, 1, 6, 4, 2, 9, 7, 8],
                [6, 4, 2, 9, 7, 8, 5, 3, 1],
                [9, 7, 8, 5, 3, 1, 6, 4, 2],
            ],
        ];

        let nr_grids = grids.len();
        // We transform the sudoku grids into an iterator of grids of field elements
        let grids_iter = (0..nr_grids * nr_random_masks_per_problem)
            .into_iter()
            .map(|_| rand::random::<[[bool; 9]; 9]>())
            .enumerate()
            .map(move |(idx, mask)| {
                let grid = grids[idx / nr_random_masks_per_problem];

                let masked_grid = core::array::from_fn(|col_idx| {
                    core::array::from_fn(|row_idx| {
                        if mask[col_idx][row_idx] {
                            0
                        } else {
                            grid[col_idx][row_idx]
                        }
                    })
                });
                (grid, masked_grid)
            });

        (symbols, grids_iter)
    }

    #[test]
    fn mock_sudoku() {
        use halo2_proofs::{dev::MockProver, pasta::Fp};

        const POW_OF_2_MAX_ROWS: u32 = 10;

        const NR_RANDOM_MASKS_PER_PROBLEM: usize = 10;

        let (symbols, sudoku_problems) = setup_values(NR_RANDOM_MASKS_PER_PROBLEM);

        // Since the search space is simply too big,
        // and there is no simple way to sample it,
        // we will use some randomness to build our
        // test cases. This makes the tests
        // non-deterministic, but also more robust,
        // since different re-runs will actually test
        // more combinations, not always the same few ones.

        for (solution, problem) in sudoku_problems {
            let circuit = SudokuCircuit::<Fp, 9, 3>::try_new(problem, solution, symbols)
                .expect("circuit generation goes wrong");

            let instance = Vec::from(problem.map(|column| Vec::from(column)));

            let prover = crate::time_it!(
                "Proof generation time: {:?}",
                MockProver::run(POW_OF_2_MAX_ROWS, &circuit, instance)
                    .expect("Proof generation goes wrong")
            );

            crate::time_it!(
                "Proof verification time: {:?}",
                prover.verify().expect("Proof verification goes wrong")
            )
        }
    }

    #[test]
    fn sudoku() {
        use crate::utilities::{ProverWrapper, VerifierWrapper};

        const POW_OF_2_MAX_ROWS: u32 = 9;

        const NR_RANDOM_MASKS_PER_PROBLEM: usize = 2;
        type TestCircuit = SudokuCircuit<Fp, 9, 3>;

        let (symbols, sudoku_problems) = setup_values(NR_RANDOM_MASKS_PER_PROBLEM);

        let circuit_wiring = TestCircuit::circuit_wiring_from_symbols(symbols);

        let mut prover =
            ProverWrapper::initialize_parameters_and_prover(POW_OF_2_MAX_ROWS, circuit_wiring)
                .expect("prover setup goes wrong");

        let sudoku_problems = Vec::from_iter(sudoku_problems);

        // Due to the awkward nested slice arguments the halo prover and verifier require,
        // we have to format the instance input.
        let instance_slices = Vec::from_iter(sudoku_problems.iter().map(|(_, problem)| {
            core::array::from_fn::<_, 9, _>(|col_idx| problem[col_idx].as_slice())
        }));

        for ((solution, problem), instance_slices) in
            sudoku_problems.iter().zip(instance_slices.iter())
        {
            let circuit = TestCircuit::try_new(problem.clone(), solution.clone(), symbols)
                .expect("creation of circuit instance should not fail");

            prover.add_item(circuit, instance_slices);
        }

        let transcript = crate::time_it! {
            "Generating proof for sudoku problems having solution takes {:?}",
            prover.prove().expect("proof generation goes wrong")
        };

        println!("The proof length is {} bytes", transcript.len());

        let mut verifier = VerifierWrapper::from(prover);

        crate::time_it! {
            "Verifying the proof that come sudoku problems have a solution takes {:?}",
            assert!(verifier.verify(
                instance_slices.iter().map(|instance| instance.as_slice()),
                &transcript
            ));
        };
    }
}
