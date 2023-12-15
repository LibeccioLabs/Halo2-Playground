use crate::{
    permutation_chip::PermutationChip, sudoku_problem_chip::SudokuProblemChip,
    utilities::RegionSequenceAssignment, Number,
};
use halo2_proofs::{
    circuit::Value,
    circuit::{Chip, Layouter, SimpleFloorPlanner},
    plonk::{Column, ConstraintSystem, Error, Fixed, Instance},
};
use std::collections::{BTreeMap, BTreeSet};
use try_collect::{ForceCollect, TryCollect};

/// A circuit that proves that the input and output values are a permutation of one another.
pub struct SudokuCircuit<F: ff::Field, const SIZE: usize, const SIZE_SQRT: usize> {
    problem: [[F; SIZE]; SIZE],
    solution: [[F; SIZE]; SIZE],
    symbols: [F; SIZE],
}

impl<F: ff::PrimeField, const SIZE: usize, const SIZE_SQRT: usize>
    SudokuCircuit<F, SIZE, SIZE_SQRT>
{
    pub fn new_unchecked(
        problem: [[F; SIZE]; SIZE],
        solution: [[F; SIZE]; SIZE],
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
            problem,
            solution,
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

impl<F: ff::Field, const SIZE: usize, const SIZE_SQRT: usize> Default
    for SudokuCircuit<F, SIZE, SIZE_SQRT>
{
    fn default() -> Self {
        Self {
            problem: [[F::ZERO; SIZE]; SIZE],
            solution: [[F::ZERO; SIZE]; SIZE],
            symbols: [F::ZERO; SIZE],
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
        Default::default()
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
        let symbol_to_ordinal = self
            .symbols
            .iter()
            .enumerate()
            .map(|(idx, sym)| (sym.to_repr().as_ref().to_owned(), idx))
            .collect::<BTreeMap<_, _>>();

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
            self.problem.map(|column| column.map(|x| Value::known(x))),
            self.solution.map(|column| column.map(|x| Value::known(x))),
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
        let mut permutation_outputs = Vec::with_capacity(3 * SIZE);

        // For each column, we obtain its permutation that aligns it to the symbols
        for col_idx in 0..SIZE {
            let col = self.solution[col_idx];
            let alloc_col = solution_cells[col_idx].clone();

            permutation_outputs.push(permutation_chip.apply_permutation(
                layouter.namespace(|| "permutating column"),
                alloc_col,
                get_permutation(col),
            )?);
        }

        // We do the same for the rows
        for row_idx in 0..SIZE {
            let row = self.solution.map(|col| col[row_idx]);
            let alloc_row = (0..SIZE)
                .map(|col_idx| solution_cells[col_idx][row_idx].clone())
                .f_collect("the number of items is correct");

            permutation_outputs.push(permutation_chip.apply_permutation(
                layouter.namespace(|| "permutating row"),
                alloc_row,
                get_permutation(row),
            )?);
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
                    .map(|(col_idx, row_idx)| self.solution[col_idx][row_idx])
                    .f_collect("the number of items is correct");
                let alloc_region = region_index_iter
                    .map(|(col_idx, row_idx)| solution_cells[col_idx][row_idx].clone())
                    .f_collect("the number of items is correct");

                permutation_outputs.push(permutation_chip.apply_permutation(
                    layouter.namespace(|| "permutating region"),
                    alloc_region,
                    get_permutation(region),
                )?);
            }
        }

        // Now we impose equality constraints among all permutation_outputs
        layouter
            .namespace(|| "permutation equality constraints")
            .assign_region(
                || "permutation equality constraints",
                |mut region| {
                    // We load the fixed symbols into the region
                    let copied_symbol_cells = (0..SIZE)
                        .map(|idx| {
                            symbol_cells[idx]
                                .0
                                .copy_advice(
                                    || "copying symbols for comparison",
                                    &mut region,
                                    permutation_chip.config().item_columns[idx].clone(),
                                    0,
                                )
                                .map(Number)
                        })
                        .try_collect::<[Number<F>; SIZE]>()
                        .map_err(|err| {
                            err.expect_try_from_error(|| "the number of items is correct")
                        })?;

                    // For each permutation result, we constrain it to be equal to the loaded symbols.
                    for p_out in permutation_outputs.iter() {
                        for (left, right) in p_out.into_iter().zip(copied_symbol_cells.iter()) {
                            region.constrain_equal(left.0.cell(), right.0.cell())?;
                        }
                    }
                    Ok(())
                },
            )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field;

    #[test]
    fn sudoku_circuit_test() {
        use halo2_proofs::{dev::MockProver, pasta::Fp};

        const POW_OF_2_MAX_ROWS: u32 = 10;
        let symbols: [Fp; 9] = (1..=9)
            .map(|n| Fp::from(n))
            .f_collect("the number of items is correct");

        let valid_sudoku9_grids: Vec<_> = [
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
        ] // We transform the sudoku grids in grids of field elements
        .into_iter()
        .map(|grid| grid.map(|col| col.map(|value| symbols[value - 1])))
        .collect();

        // Since the search space is simply too big,
        // and there is no simple way to sample it,
        // we will use some randomness to build our
        // test cases. This makes the tests
        // non-deterministic, but also more robust,
        // since different re-runs will actually test
        // more combinations, not always the same few ones.

        for solved_sudoku in valid_sudoku9_grids {
            for _ in 0..10 {
                let mask = rand::random::<[[bool; 9]; 9]>();

                let problem = mask
                    .into_iter()
                    .zip(solved_sudoku)
                    .map(|(mask_col, problem_col)| {
                        mask_col
                            .into_iter()
                            .zip(problem_col)
                            .map(|(mask_it, n)| if mask_it { Fp::ZERO } else { n })
                            .f_collect::<[Fp; 9]>("the number of items is correct")
                    })
                    .f_collect("the number of items is correct");

                let circuit =
                    SudokuCircuit::<Fp, 9, 3>::new_unchecked(problem, solved_sudoku, symbols);

                let instance = Vec::from(problem.map(|column| Vec::from(column)));

                let t0 = std::time::Instant::now();
                let prover = MockProver::run(POW_OF_2_MAX_ROWS, &circuit, instance)
                    .expect("Proof generation goes wrong");
                let t1 = std::time::Instant::now();
                println!("Proof generation time: {:?}", t1 - t0);
                let t2 = std::time::Instant::now();
                prover.verify().expect("Proof verification goes wrong");
                let t3 = std::time::Instant::now();
                println!("Proof verification time: {:?}", t3 - t2);
            }
        }
    }
}
