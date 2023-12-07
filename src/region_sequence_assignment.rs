use crate::Number;
use halo2_proofs::{
    circuit::{Region, Value},
    plonk::{Any, Column, ColumnType, Error},
};

// The implementation of this TryFrom is motivated by
// the extensive use of traits from the `try_collect` crate.
//
// Indeed, the cell assignment operations return a result,
// and in many occasions we would like to perform such assignments
// for fixed-length many elements, to be collected into arrays.
//
// Result normally does not implement TryInto for its Ok type,
// and we fix this inconvenience here, rather trivially
impl<E, F: ff::Field> TryFrom<Result<Number<F>, E>> for Number<F> {
    type Error = E;
    fn try_from(value: Result<Number<F>, E>) -> Result<Self, Self::Error> {
        value
    }
}

// For the same reason, we implement the trait for arrays of numbers.
// Since arrays are always foreign types, we have to wrap them to implement
// foreign traits on them.
#[repr(transparent)]
struct ArrayWrap<T, const LEN: usize>(pub [T; LEN]);

impl<E, T, const LEN: usize> TryFrom<Result<[T; LEN], E>> for ArrayWrap<T, LEN> {
    type Error = E;
    fn try_from(value: Result<[T; LEN], E>) -> Result<Self, Self::Error> {
        value.map(ArrayWrap)
    }
}

impl<T, const LEN: usize> Into<[T; LEN]> for ArrayWrap<T, LEN> {
    fn into(self) -> [T; LEN] {
        self.0
    }
}

pub trait RegionSequenceAssignment<F: ff::Field> {
    /// Given a region, a column, an offset, and an array of values,
    /// this function assigns the values of the array to cells in the given
    /// column, with relative row index `[offset .. offset + LEN]`
    fn assign_array_to_column<const LEN: usize, CT: ColumnType>(
        &mut self,
        column: Column<CT>,
        offset: usize,
        to_column_values: [Value<F>; LEN],
    ) -> Result<[Number<F>; LEN], Error>
    where
        Column<CT>: Into<Column<Any>>;

    /// Given a region, an array of columns, an offset, and
    /// an array of arrays of values,
    /// this function assigns the values of the array to cells in the given
    /// columns, with relative row index `[offset .. offset + ROW_NR]`
    fn assign_grid_to_columns<const COLUMN_NR: usize, const ROW_NR: usize, CT: ColumnType>(
        &mut self,
        columns: [Column<CT>; COLUMN_NR],
        offset: usize,
        grid_values: [[Value<F>; ROW_NR]; COLUMN_NR],
    ) -> Result<[[Number<F>; ROW_NR]; COLUMN_NR], Error>
    where
        Column<CT>: Into<Column<Any>>;
}

use try_collect::{ForceCollect, TryCollect};

impl<'a, F: ff::Field> RegionSequenceAssignment<F> for Region<'a, F> {
    fn assign_array_to_column<const LEN: usize, CT: ColumnType>(
        &mut self,
        column: Column<CT>,
        offset: usize,
        to_column_values: [Value<F>; LEN],
    ) -> Result<[Number<F>; LEN], Error>
    where
        Column<CT>: Into<Column<Any>>,
    {
        let ann = || "assigning array to column";
        let row_and_value_iter = (offset..offset + LEN).zip(to_column_values);
        // A lot of the code duplication happening here is not avoidable due to
        // functions in Rust having different types.
        // The duplication is needed because in one branch we use
        // `self.assign_advice`, and in the other `self.assign_fixed`,
        // which require different kinds of column as arguments.
        match (*column.column_type()).into() {
            Any::Advice => {
                let column = column.into().try_into().unwrap();
                row_and_value_iter
                    .map(|(row_idx, value)| {
                        self.assign_advice(ann, column, row_idx, || value)
                            .map(Number)
                    })
                    .try_collect::<[Number<F>; LEN]>()
            }
            Any::Fixed => {
                let column = column.into().try_into().unwrap();
                row_and_value_iter
                    .map(|(row_idx, value)| {
                        self.assign_fixed(ann, column, row_idx, || value)
                            .map(Number)
                    })
                    .try_collect::<[Number<F>; LEN]>()
            }
            Any::Instance => unimplemented!("idk how to do it, and i think i won't need it"),
        }
        .map_err(|err| err.expect_try_from_error(|| "we know the number of items is correct"))
    }

    fn assign_grid_to_columns<const COLUMN_NR: usize, const ROW_NR: usize, CT: ColumnType>(
        &mut self,
        columns: [Column<CT>; COLUMN_NR],
        offset: usize,
        grid_values: [[Value<F>; ROW_NR]; COLUMN_NR],
    ) -> Result<[[Number<F>; ROW_NR]; COLUMN_NR], Error>
    where
        Column<CT>: Into<Column<Any>>,
    {
        columns
            .into_iter()
            .zip(grid_values)
            .map(|(column, values)| Self::assign_array_to_column(self, column, offset, values))
            .try_collect::<[ArrayWrap<Number<F>, ROW_NR>; COLUMN_NR]>()
            .map_err(|err| err.expect_try_from_error(|| "we know the number of items is correct"))
            .map(|grid| grid.f_collect("the number of items is correct"))
    }
}
