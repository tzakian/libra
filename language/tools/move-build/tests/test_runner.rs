// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use move_build::{
    resolution::resolution_graph as RG, source_package::manifest_parser as MP, BuildConfig,
};
use move_command_line_common::testing::{format_diff, read_env_update_baseline, EXP_EXT};
use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

pub fn run_test(path: &Path) -> datatest_stable::Result<()> {
    let update_baseline = read_env_update_baseline();
    if path
        .components()
        .any(|component| component == Component::Normal(OsStr::new("deps_only")))
    {
        return Ok(());
    }
    let exp_path = path.with_extension(EXP_EXT);

    let exp_exists = exp_path.is_file();

    let contents = fs::read_to_string(path)?;
    let output = match MP::parse_move_manifest_string(contents)
        .and_then(MP::parse_source_manifest)
        .and_then(|parsed_manifest| {
            RG::ResolutionGraph::new(
                parsed_manifest,
                path.parent().unwrap().to_path_buf(),
                BuildConfig {
                    dev_mode: true,
                    generate_transaction_builders: false,
                    generate_abis: false,
                },
            )
        })
        .and_then(|rg| rg.resolve())
    {
        Ok(mut resolved_package) => {
            for (_, package) in resolved_package.package_table.iter_mut() {
                package.package_path = PathBuf::from("ELIDED_FOR_TEST");
            }
            format!("{:#?}", resolved_package)
        }
        Err(error) => format!("{:#}", error),
    };

    if update_baseline {
        fs::write(&exp_path, &output)?;
        return Ok(());
    }

    if exp_exists {
        let expected = fs::read_to_string(&exp_path)?;
        if expected != output {
            return Err(anyhow::format_err!(
                "Expected outputs differ for {:?}:\n{}",
                exp_path,
                format_diff(expected, output)
            )
            .into());
        }
    } else {
        return Err(anyhow::format_err!(
            "No expected output found for {:?}.\
                    You probably want to rerun with `env UPDATE_BASELINE=1`",
            path
        )
        .into());
    }
    Ok(())
}

datatest_stable::harness!(run_test, "tests/test_sources", r".*\.toml$");