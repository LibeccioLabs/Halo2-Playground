#[macro_export]
macro_rules! token_times_sequence {
    ([$tok: tt][$($seq: tt),*]) => {
        token_times_sequence!([$tok][$($seq),*][])
    };
    (
        [$tok: tt]
        [$s_first: tt $(, $s_others: tt)*]
        [$(($tok_cpy: tt, $seq_item: tt)),+]
    ) => {
        token_times_sequence!(
            [$tok]
            [$($s_others),*]
            [$(($tok_cpy, $seq_item)),+ , ($tok, $s_first)]
        )
    };
    (
        [$tok: tt]
        [$s_first: tt $(, $s_others: tt)*]
        []
    ) => {
        token_times_sequence!(
            [$tok]
            [$($s_others),*]
            [($tok, $s_first)]
        )
    };
    ([$tok: tt][] $result: tt) => {$result};
}

#[macro_export]
macro_rules! sequence_product {
    ([$($s1: tt),*][$($s2: tt),*]) => {
        _inner_sequence_product!([$($s1),*][$($s2),*][])
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _inner_sequence_product {
    (
        [$s1_first: tt $(, $s1_other: tt)*]
        $s2: tt
        []
    ) => {
        _inner_sequence_product!(
            [$($s1_other),*]
            $s2
            [token_times_sequence!([$s1_first] $s2)]
        )
    };
    (
        [$s1_first: tt $(, $s1_other: tt)*]
        $s2: tt
        [$($partial_result: expr),+]
    ) => {
        _inner_sequence_product!(
            [$($s1_other),*]
            $s2
            [$($partial_result),+ , token_times_sequence!([$s1_first] $s2)]
        )
    };
    ([] $s2: tt $result: tt) => {$result};
}

/// A macro that, given the name of another macro, and given
/// a list of lists of expressions,
/// calls the given macro with every possible combination of items
/// from the given lists.
///
/// ## Call Syntax
///
/// In order to call the macro, the arguments have to be formatted as follows:
///
/// `$macro_name: path ; $([ $($seq: expr,)+ ])+`
///
/// ## Example:
///
/// The following code
///
/// ```ignore
/// iter_apply_macro!(
///     println ;
///     ["{} {}!"]
///     ["hello", "greetings"]
///     ["world", "mom"]
/// );
/// ```
///
/// expands to
///
/// ``` ignore
/// println!("{} {}!", "hello", "world");
/// println!("{} {}!", "hello", "mom");
/// println!("{} {}!", "greetings", "world");
/// println!("{} {}!", "greetings", "mom");
/// ```
#[macro_export]
macro_rules! iter_apply_macro {
    ($macro_name: path; $([$($seq: expr),+])+) => {
        crate::_inner_iter_apply_macro!($macro_name; {} $([$($seq,)+])+);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _inner_iter_apply_macro {
    (
        $macro_name: path;
        {$($params_list: expr,)*}
        [$seq1_first: expr, $($seq1: expr,)*]
        $($other_seq: tt)*
    ) => {
        crate::_inner_iter_apply_macro!(
            $macro_name;
            {$($params_list,)* $seq1_first, }
            $($other_seq)*
        );
        crate::_inner_iter_apply_macro!(
            $macro_name;
            {$($params_list,)*}
            [$($seq1,)*]
            $($other_seq)*
        );
    };
    (
        $macro_name: path;
        {$($params_list: expr,)*}
        []
        $($other_seq: tt)*
    ) => {};
    (
        $macro_name: path;
        {$($params_list: expr,)*}
    ) => {
        $macro_name ! ($($params_list),*);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn token_times_sequence_test() {
        let v = token_times_sequence!([42][1, 2, 3, 4]);
        assert_eq!(v, [(42, 1), (42, 2), (42, 3), (42, 4)]);
    }

    #[test]
    fn sequence_product_test() {
        let v = sequence_product!([1, 2, 3][4, 5, 6]);
        assert_eq!(
            v,
            [
                [(1, 4), (1, 5), (1, 6)],
                [(2, 4), (2, 5), (2, 6)],
                [(3, 4), (3, 5), (3, 6)]
            ]
        );
    }

    #[test]
    fn iter_apply_macro_test() {
        iter_apply_macro!(
            println;
            ["{:?} {}", "{:#?} {:?}"]
            [vec!["lol", "asd"], 2_i32.pow(10)]
            ["mom", String::from("dad")]
        );
    }
}
