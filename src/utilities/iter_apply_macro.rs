/// A macro that, given the name of another macro, and given
/// a list of lists of expressions,
/// calls the given macro with every possible combination of items
/// from the given lists.
///
/// ## Call Syntax
///
/// In order to call the macro, the arguments have to be formatted as follows:
///
/// `$macro_name: path ; $([ $($seq: expr),+ ])+`
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
    fn iter_apply_macro_test() {
        macro_rules! format_push {
            ($vec: expr, $($format_args: tt)*) => {
                $vec.push(format!($($format_args)*));
            };
        }

        let mut v = vec![];

        iter_apply_macro!(
            format_push;
            [&mut v]
            ["{:?} {}", "{:#?} {:?}"]
            [vec!["lol", "asd"], 2_i32.pow(10)]
            ["mom", String::from("dad")]
        );

        assert_eq!(
            v,
            vec![
                "[\"lol\", \"asd\"] mom",
                "[\"lol\", \"asd\"] dad",
                "1024 mom",
                "1024 dad",
                "[\n    \"lol\",\n    \"asd\",\n] \"mom\"",
                "[\n    \"lol\",\n    \"asd\",\n] \"dad\"",
                "1024 \"mom\"",
                "1024 \"dad\""
            ]
        );
    }
}
