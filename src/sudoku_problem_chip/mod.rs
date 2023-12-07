use halo2_proofs::{
    circuit::{Chip, Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Selector},
    poly::Rotation,
};

use crate::region_sequence_assignment::RegionSequenceAssignment;

use super::Number;

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

// TODO: I have no clue what this trait does yet.
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

impl<const SIZE: usize, F: ff::Field> SudokuProblemChip<SIZE, F> {
    pub fn construct(config: SPConfig<SIZE>) -> Self {
        Self {
            config,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        grid_columns: [Column<Advice>; SIZE],
    ) -> <Self as halo2_proofs::circuit::Chip<F>>::Config {
        for col in grid_columns {
            meta.enable_equality(col);
        }

        let s_grid_compatibility = meta.selector();

        meta.create_gate("compatibility between sudoku grid and solution", |meta| {
            let mut constraints = vec![];

            let s_grid_compatibility = meta.query_selector(s_grid_compatibility);

            for col in grid_columns {
                for row_idx in 0..SIZE {
                    let problem_cell = meta.query_advice(col, Rotation(row_idx as i32));
                    let solution_cell = meta.query_advice(col, Rotation((row_idx + SIZE) as i32));
                    // if s_grid_eq is enabled, then either
                    // - problem_cell is 0, or
                    // - problem_cell == solution_cell
                    constraints.push(
                        s_grid_compatibility.clone()
                            * problem_cell.clone()
                            * (problem_cell - solution_cell),
                    );
                }
            }

            constraints
        });

        SPConfig {
            grid_columns,
            s_grid_compatibility,
        }
    }

    /// Loads `problem_grid_inputs` and `solution_grid_inputs`
    /// and enforces their compatibility by activating the gate associated
    /// to this circuit.
    pub fn enforce_grid_compatibility(
        &self,
        mut layouter: impl Layouter<F>,
        problem_grid_inputs: [[Value<F>; SIZE]; SIZE],
        solution_grid_inputs: [[Value<F>; SIZE]; SIZE],
    ) -> Result<SudokuProblemAssignment<SIZE, F>, Error> {
        let config = self.config();

        layouter.assign_region(
            || "load problem-solution sudoku grids",
            |mut region| {
                // enables the chip's gate. This makes it so that the constraints are
                // actually checked for the grids we are going to load
                config.s_grid_compatibility.enable(&mut region, 0)?;

                let columns = config.grid_columns;

                let problem_grid =
                    region.assign_grid_to_columns(columns, 0, problem_grid_inputs)?;
                let solution_grid =
                    region.assign_grid_to_columns(columns, SIZE, solution_grid_inputs)?;
                Ok(SudokuProblemAssignment {
                    problem_grid,
                    solution_grid,
                })
            },
        )
    }
}
