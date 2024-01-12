use super::*;

impl<const N_OBJECTS: usize, F: ff::Field> PermutationChip<N_OBJECTS, F> {
    pub fn construct(config: <Self as halo2_proofs::circuit::Chip<F>>::Config) -> Self {
        Self {
            config,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        item_columns: [Column<Advice>; N_OBJECTS],
        swap_selector_columns: Vec<Column<Advice>>,
    ) -> <Self as halo2_proofs::circuit::Chip<F>>::Config {
        assert!(
            !swap_selector_columns.is_empty(),
            "At least one column to allocate swap selectors is needed."
        );

        for col in item_columns {
            meta.enable_equality(col);
        }

        let s_perm = meta.selector();

        // The initial position of input items.
        let mut output_item_positions: [_; N_OBJECTS] =
            core::array::from_fn(|idx| (item_columns[idx], Rotation::cur()));

        meta.create_gate("object permutation", |meta| {
            let mut constraints = vec![];

            let s_perm = meta.query_selector(s_perm);
            let swap_schedule = bubble_sort_swap_schedule::<N_OBJECTS>();
            let mut items_tracker: [halo2_proofs::plonk::Expression<F>; N_OBJECTS] = item_columns
                .iter()
                .map(|column| meta.query_advice(*column, Rotation::cur()))
                .f_collect("number of items is correct");

            // Unfortunately we need this refcell because otherwise the compiler complains
            // about meta being used in two closures at the same time.
            let ask_meta = RefCell::new(meta);

            // An iterator that visits the cells of `item_columns`, row by row.
            // The first row is entirely occupied by the input items of the circuit,
            // so the iterator skips it.
            let mut next_free_cell =
                DivModCounter::<N_OBJECTS>::new_const_mod(1, 0).map(|(row_idx, col_idx)| {
                    (
                        item_columns[col_idx],
                        Rotation(row_idx as i32),
                        ask_meta
                            .borrow_mut()
                            .query_advice(item_columns[col_idx], Rotation(row_idx as i32)),
                    )
                });

            // An iterator that visits the cells of `swap_selector_columns`, row by row.
            let mut next_swap_selector =
                DivModCounter::<0, true>::new_runtime_mod(0, 0, swap_selector_columns.len()).map(
                    |(row_idx, col_idx)| {
                        ask_meta
                            .borrow_mut()
                            .query_advice(swap_selector_columns[col_idx], Rotation(row_idx as i32))
                    },
                );

            for (swap_idx1, swap_idx2) in swap_schedule {
                let (next_idx1_col, next_idx1_row, next_idx1_value) =
                    next_free_cell.next().expect("the iterator never ends");
                let (next_idx2_col, next_idx2_row, next_idx2_value) =
                    next_free_cell.next().expect("the iterator never ends");

                let swap_selector = next_swap_selector.next().expect("the iterator never ends");

                // swap_selector must be a boolean value
                constraints.push(
                    s_perm.clone()
                        * swap_selector.clone()
                        * (swap_selector.clone() - Expression::Constant(F::ONE)),
                );

                // if swap_selector is 0, then we want
                // `next_idx1_cell == item_tracker[swap_idx1]` and
                // `next_idx2_cell == item_tracker[swap_idx2]`
                //
                // if swap_selector is 1, then we want
                // `next_idx1_cell == item_tracker[swap_idx2]` and
                // `next_idx2_cell == item_tracker[swap_idx1]`

                constraints.push(
                    s_perm.clone()
                        * (items_tracker[swap_idx1].clone() - next_idx1_value.clone()
                            + swap_selector.clone()
                                * (next_idx1_value.clone() - next_idx2_value.clone())),
                );
                constraints.push(
                    s_perm.clone()
                        * (items_tracker[swap_idx2].clone() - next_idx2_value.clone()
                            + swap_selector.clone()
                                * (next_idx2_value.clone() - next_idx1_value.clone())),
                );

                // now we update `item_tracker`
                items_tracker[swap_idx1] = next_idx1_value;
                items_tracker[swap_idx2] = next_idx2_value;

                // we also update `output_item_positions`
                // so that whoever uses this chip will be able to locate where
                // the output items are.
                output_item_positions[swap_idx1] = (next_idx1_col, next_idx1_row);
                output_item_positions[swap_idx2] = (next_idx2_col, next_idx2_row);
            }

            constraints
        });

        PConfig {
            item_columns,
            swap_selector_columns,
            output_item_positions,
            s_perm,
        }
    }
}
