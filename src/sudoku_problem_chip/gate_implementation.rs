use super::*;

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
}
