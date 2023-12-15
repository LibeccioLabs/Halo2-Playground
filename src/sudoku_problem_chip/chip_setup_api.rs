use super::*;

impl<const SIZE: usize, F: ff::Field> SudokuProblemChip<SIZE, F> {
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
