// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

#[macro_export]
macro_rules! ty_constraint {
    ($x:ident => $constraint:expr) => {{
        Box::new(move |$x| $constraint)
    }};
}

#[macro_export]
macro_rules! eff {
    ($sender:ident, $args:ident, $ty_args:ident => $eff:expr) => {
        Box::new(move |$sender, $args, $ty_args| $eff)
    };
}
