use super::*;

impl<const N_OBJECTS: usize, F: ff::Field> PermutationChip<N_OBJECTS, F> {
    /// Loads `input_items` in the circuit, applies `permutation` to them,
    /// and upon successful execution, returns `Ok(array)`,
    /// where `array` is an array of cells that represent `input_items`
    /// after `permutation` has been applied to them,
    /// i.e. such that, for every `i`, it holds
    /// `array[i] = input_items[permutation[i]]`
    pub fn apply_permutation(
        &self,
        mut layouter: impl Layouter<F>,
        input_items: [Number<F>; N_OBJECTS],
        permutation: [usize; N_OBJECTS],
    ) -> Result<[Number<F>; N_OBJECTS], Error> {
        layouter.assign_region(
            || "load input",
            |region| apply_permutation_region_assignment(self, &input_items, permutation, region),
        )
    }
}

/// A helper function to be used in
/// `PermutationChip::<N_OBJECTS, F>::apply_permutation`.
/// Its main purpose is to increase readability by reducing indentation.
fn apply_permutation_region_assignment<const N_OBJECTS: usize, F: ff::Field>(
    chip: &PermutationChip<N_OBJECTS, F>,
    input_items: &[Number<F>; N_OBJECTS],
    permutation: [usize; N_OBJECTS],
    mut region: Region<'_, F>,
) -> Result<[Number<F>; N_OBJECTS], Error> {
    // We enable the selector gate that activates all the constraints in
    // the permutation chip.
    chip.config.s_perm.enable(&mut region, 0)?;

    // We load the input cells in the first row of the region.
    for (idx, input_item) in input_items.iter().enumerate().take(N_OBJECTS) {
        input_item.copy_advice(
            || "input items",
            &mut region,
            chip.config.item_columns[idx],
            0,
        )?;
    }

    // For every swap in the schedule, we fill the cells in the region,
    // as required by the constraints in the "object permutation" gate.

    // The first row is entirely occupied by the input items of the circuit,
    // so the iterator skips it.
    let mut next_free_cell = DivModCounter::<N_OBJECTS>::new_const_mod(1, 0)
        .map(|(row_idx, col_idx)| (chip.config.item_columns[col_idx], row_idx));

    // An iterator that visits the cells of `swap_selector_columns`, row by row.
    let mut next_swap_selector =
        DivModCounter::new_runtime_mod(0, 0, chip.config.swap_selector_columns.len())
            .map(|(row_idx, col_idx)| (chip.config.swap_selector_columns[col_idx], row_idx));

    let mut item_tracker: [Number<F>; N_OBJECTS] = (0..N_OBJECTS)
        .map(|idx| input_items[idx].clone())
        .f_collect("the number of items is correct");

    for (swap_is_applied, idx1, idx2) in from_permutation_to_bubble_sort_swap_schedule(permutation)
    {
        let (col1, row1) = next_free_cell.next().expect("the iterator never ends");
        let (col2, row2) = next_free_cell.next().expect("the iterator never ends");
        let (s_col, s_row) = next_swap_selector.next().expect("the iterator never ends");

        if swap_is_applied {
            item_tracker.swap(idx1, idx2);
        }

        // We assign the swap result in the next free cells
        // Also, we assign the result of the assignment back into the item
        // tracker, so that we will know where to locate the output values
        // at the end of the procedure.
        item_tracker[idx1] = region
            .assign_advice(
                || {
                    format!(
                        "{}-th value after swap for indices {}, {}",
                        idx1, idx1, idx2
                    )
                },
                col1,
                row1,
                || item_tracker[idx1].value().copied(),
            )
            .map(Number)?;
        item_tracker[idx2] = region
            .assign_advice(
                || {
                    format!(
                        "{}-th value after swap for indices {}, {}",
                        idx2, idx1, idx2
                    )
                },
                col2,
                row2,
                || item_tracker[idx2].value().copied(),
            )
            .map(Number)?;

        // We assign the boolean value that will be used by the constraint
        // system to enforce the swaps
        region.assign_advice(
            || format!("swap selector for indices {}, {}", idx1, idx2),
            s_col,
            s_row,
            || Value::known(if swap_is_applied { F::ONE } else { F::ZERO }),
        )?;
    }
    Ok(item_tracker)
}
