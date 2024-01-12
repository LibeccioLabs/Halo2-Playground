use try_collect::ForceCollect;

/// A struct that iterates over all the permutations of a given length.
pub struct PermutationsIter<const N_OBJECTS: usize>;

impl<const N_OBJECTS: usize> IntoIterator for PermutationsIter<N_OBJECTS> {
    type IntoIter = KnuthL<N_OBJECTS>;
    type Item = [usize; N_OBJECTS];
    fn into_iter(self) -> Self::IntoIter {
        KnuthL::<N_OBJECTS>::default()
    }
}

/// A struct that iterates over all the permutations of a given length.
pub struct KnuthL<const N_OBJECTS: usize>(Option<[usize; N_OBJECTS]>);

impl<const N_OBJECTS: usize> Default for KnuthL<N_OBJECTS> {
    fn default() -> Self {
        if N_OBJECTS == 0 {
            return Self(None);
        }

        Self(Some(
            (0..N_OBJECTS).f_collect("the number of items is correct"),
        ))
    }
}

impl<const N_OBJECTS: usize> Iterator for KnuthL<N_OBJECTS> {
    type Item = [usize; N_OBJECTS];
    fn next(&mut self) -> Option<Self::Item> {
        self.0?; // return None if None

        // Copy the current state, as return value.
        let current = self.0;

        let array = self
            .0
            .as_mut()
            .expect("we checked at the beginning that self.0 != None");

        // Find last j such that self[j] <= self[j+1].
        // Nullify self.0 if it doesn't exist
        let j = (0..=N_OBJECTS - 2)
            .rev()
            .find(|&j| array[j] <= array[j + 1]);

        // The last permutation we yield is [N_OBJECTS - 1, N_OBJECTS - 2, ..., 1, 0]
        if j.is_none() {
            self.0 = None;
            return current;
        }
        let j = j.unwrap();

        // Find last l such that self[j] <= self[l], then
        // exchange elements j and l, and then reverse self[j+1..]
        let l = (j + 1..=N_OBJECTS - 1).rev().find(|&l| array[j] <= array[l])
        .expect("since `j + 1` is in the range, and given the definition of `j`, we are sure that `find` will return `Some(...)`");
        array.swap(j, l);
        array[j + 1..].reverse();

        current
    }
}

/// Given a permutation, outputs its inverse.
/// It is up to the caller to guarantee that the input
/// to this function is an actual permutation.
pub fn inverse_permutation<const N_OBJECTS: usize>(
    permutation: [usize; N_OBJECTS],
) -> [usize; N_OBJECTS] {
    let mut output = [0; N_OBJECTS];
    for (i, n) in permutation.into_iter().enumerate() {
        output[n] = i;
    }
    output
}
