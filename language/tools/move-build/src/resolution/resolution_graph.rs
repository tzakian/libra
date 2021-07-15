use crate::{
    source_package::{
        layout::SourcePackageLayout,
        manifest_parser::{parse_move_manifest_string, parse_source_manifest},
        parsed_manifest::{
            AddressDeclarations, Dependency, DevAddressDeclarations, SourceManifest, SubstOrRename,
        },
    },
    BuildConfig,
};
use anyhow::{bail, Context, Result};
use move_core_types::{
    account_address::AccountAddress,
    identifier::{IdentStr, Identifier},
};
use petgraph::{graph::NodeIndex, Directed, Graph};
use std::{collections::BTreeMap, path::PathBuf};

pub type ResolutionTable = BTreeMap<Identifier, Option<AccountAddress>>;
// rename_to => (package name, address name)
pub type Renaming = BTreeMap<Identifier, (Identifier, Identifier)>;
pub type GraphIndex = NodeIndex<u32>;

#[derive(Debug, Clone)]
pub struct ResolutionGraph {
    // Build options
    pub build_options: BuildConfig,
    // Root package
    pub root_package: SourceManifest,
    // Dependency graph
    pub graph: Graph<Identifier, Identifier, Directed>,
    // A mapping of package name to its resolution
    pub package_table: BTreeMap<Identifier, ResolutionPackage>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolutionPackage {
    // Pointer into the `ResolutionGraph.graph`
    pub resolution_graph_index: GraphIndex,
    // source manifest for this package
    pub source_package: SourceManifest,
    // Where this package is located on the filesystem
    pub package_path: PathBuf,
    // Th renaming of addresses performed by this package
    pub renaming: Renaming,
    // The mapping of addresses for this package (and that are in scope for it)
    pub resolution_table: ResolutionTable,
}

impl ResolutionGraph {
    pub fn new(
        root_package: SourceManifest,
        build_options: BuildConfig,
    ) -> Result<ResolutionGraph> {
        let mut resolution_graph = Self {
            build_options,
            root_package: root_package.clone(),
            graph: Graph::new(),
            package_table: BTreeMap::new(),
        };
        resolution_graph
            .build_resolution_graph(root_package.clone(), std::env::current_dir().unwrap())
            .with_context(|| {
                format!(
                    "Unable to resolve packages for {}",
                    root_package.package.name
                )
            })?;
        let (root_renaming, root_resolution_table) = {
            let resolved_root =
                &resolution_graph.package_table[&resolution_graph.root_package.package.name];
            (
                resolved_root.renaming.clone(),
                resolved_root.resolution_table.clone(),
            )
        };

        for dep_name in root_package.dependencies.keys() {
            resolution_graph
                .unify_over_graph(dep_name, &root_renaming, &root_resolution_table)
                .with_context(|| format!("While finishing resolution of package {}", dep_name))?;
        }
        Ok(resolution_graph)
    }

    fn build_resolution_graph(
        &mut self,
        package: SourceManifest,
        package_path: PathBuf,
    ) -> Result<()> {
        let package_name = package.package.name.clone();
        let package_node_id = match self.package_table.get(&package_name) {
            None => self.graph.add_node(package_name.clone()),
            // Same package: OK
            Some(other) if other.source_package == package => other.resolution_graph_index,
            // Different packages, with same name: Not OK
            Some(other) => {
                bail!(
                    "Conflicting dependencies found for package {}: {:#?} conflicts with {:#?}",
                    other.source_package.package.name,
                    package,
                    other.source_package,
                )
            }
        };

        let mut renaming = BTreeMap::new();
        let mut resolution_table = BTreeMap::new();
        for (dep_name, dep) in package.dependencies.clone() {
            let (dep_renaming, dep_resolution_table, dep_node_idx) = self
                .process_dependency(dep_name.clone(), dep, package_path.clone())
                .with_context(|| {
                    format!(
                        "While resolving dependency {} in package {}",
                        dep_name, package_name
                    )
                })?;
            ResolutionPackage::extend_renaming(&mut renaming, &dep_name, dep_renaming)
                .with_context(|| {
                    format!(
                        "While resolving address renames in dependency {} in package {}",
                        dep_name, package_name
                    )
                })?;
            ResolutionPackage::extend_resolution_table(
                &mut resolution_table,
                &dep_name,
                dep_resolution_table,
            )
            .with_context(|| {
                format!(
                    "Resolving named addresses for dependency '{}' in package '{}'",
                    dep_name, package_name
                )
            })?;
            self.graph.add_edge(package_node_id, dep_node_idx, dep_name);
        }

        resolution_table.extend(package.addresses.clone().unwrap_or_else(BTreeMap::new));

        let resolved_package = ResolutionPackage {
            resolution_graph_index: package_node_id,
            source_package: package,
            package_path: package_path.clone().canonicalize()?,
            renaming,
            resolution_table,
        };

        //for dep_name in resolved_package.source_package.dependencies.keys() {
        //self.unify_over_graph(dep_name, &resolved_package.renaming, &resolved_package.resolution_table)?;
        //}

        self.package_table.insert(package_name, resolved_package);
        Ok(())
    }

    fn unify_over_graph(
        &mut self,
        dep_name: &IdentStr,
        renaming: &Renaming,
        resolution: &ResolutionTable,
    ) -> Result<()> {
        println!("--------------------");
        println!("dep_resolution_before: {:#?}", self.package_table[dep_name]);
        self.package_table
            .get_mut(dep_name)
            .unwrap()
            .resolve(resolution, renaming)
            .with_context(|| format!("Unable to resolve addresses in dependency {}", dep_name))?;
        let dep_resolution = self.package_table.get(dep_name).unwrap().clone();
        println!(
            "dep_name: {}\nRenaming: {:#?}\nResolution: {:#?}",
            dep_name, renaming, resolution
        );
        println!(
            "dep resolution after: {:#?}",
            dep_resolution.resolution_table
        );
        println!(">> --------------------\n");

        for package_dep_name in dep_resolution.source_package.dependencies.keys() {
            self.unify_over_graph(
                package_dep_name,
                &dep_resolution.renaming,
                &dep_resolution.resolution_table,
            )
            .with_context(|| {
                format!(
                    "Resolving dependency {} in package {} causes a resolution conflict",
                    package_dep_name, dep_name
                )
            })?;
        }
        Ok(())
    }

    fn process_dependency(
        &mut self,
        dep_name: Identifier,
        dep: Dependency,
        root_path: PathBuf,
    ) -> Result<(Renaming, ResolutionTable, GraphIndex)> {
        let (dep_package, dep_package_dir) = Self::parse_package_manifest(&dep, root_path)
            .with_context(|| format!("While processing dependency {}", dep_name))?;
        self.build_resolution_graph(dep_package.clone(), dep_package_dir)
            .with_context(|| format!("Unable to resolve package dependency for {}", dep_name))?;
        let dep_node_id = self.package_table[&dep_package.package.name].resolution_graph_index;
        if dep_name != dep_package.package.name {
            bail!("Name of dependency declared in package ('{}') does not match package name of dependency ('{}')",
            dep_name,
            dep_package.package.name
            );
        }

        let resolved_dep = &self.package_table[&dep_name];
        let mut renaming = BTreeMap::new();

        let mut resolution_table = resolved_dep.resolution_table.clone();

        // check that address being renamed exists in the dep that is being renamed/imported
        if let Some(dep_subst) = dep.subst {
            for (name, rename_from_or_assign) in dep_subst.into_iter() {
                match rename_from_or_assign {
                    SubstOrRename::RenameFrom(ident) => {
                        if !resolved_dep.resolution_table.contains_key(&ident) {
                            bail!("Tried to rename named address {0} from package '{1}'. However, {1} does not contain that address.",
                                ident, dep_name
                            );
                        }

                        // Apply the substitution
                        if let Some(other_val) = resolution_table.remove(&ident) {
                            resolution_table.insert(name.clone(), other_val);
                        }

                        if let Some(_) = renaming.insert(name.clone(), (dep_name.clone(), ident)) {
                            bail!("Duplicate renaming of named address '{0}' found for dependency {1}",
                                name,
                                dep_name,
                            );
                        }
                    }
                    SubstOrRename::Assign(value) => {
                        if let Some(Some(_)) = resolution_table.insert(name.clone(), Some(value)) {
                            bail!(
                                "Named address assignment conflict for {} in dependency {}'",
                                name,
                                dep_name,
                            );
                        }
                    }
                }
            }
        }

        Ok((renaming, resolution_table, dep_node_id))
    }

    fn parse_package_manifest(
        dep: &Dependency,
        mut root_path: PathBuf,
    ) -> Result<(SourceManifest, PathBuf)> {
        root_path.push(&dep.local);
        match std::fs::read_to_string(&root_path.join(SourcePackageLayout::Manifest.path())) {
            Ok(contents) => {
                let source_package: SourceManifest =
                    parse_move_manifest_string(contents).and_then(parse_source_manifest)?;
                Ok((source_package, root_path))
            }
            Err(_) => Err(anyhow::format_err!(
                "Unable to find package manifest at {:?}",
                SourcePackageLayout::Manifest.path().join(root_path),
            )),
        }
    }
}

impl ResolutionPackage {
    fn extend_renaming(
        renaming: &mut Renaming,
        dep_name: &IdentStr,
        dep_renaming: Renaming,
    ) -> Result<()> {
        // 1. check for duplicate names in rename_to
        for (rename_to, rename_from) in dep_renaming.into_iter() {
            if let Some(_) = renaming.insert(rename_to.clone(), rename_from) {
                bail!(
                    "Duplicate renaming of {} found in dependency {}",
                    rename_to,
                    dep_name
                );
            }
        }
        Ok(())
    }

    // resolve the package `self` based on the resolution table and renamings that are done in the
    // package that pulls it in. ("pushing down instantiations").
    fn resolve(
        &mut self,
        resolution_context: &ResolutionTable,
        renaming_context: &Renaming,
    ) -> Result<()> {
        // These are the addrs that we want to look at in the `resolution_context` to see if they
        // have values, and then resolve and apply the value to this package.
        let addrs = renaming_context
            .iter()
            .filter_map(|(renaming, (from_package_name, from_package_addr_name))| {
                println!(
                    "[{}]: {} <- ({}, {})",
                    self.source_package.package.name,
                    renaming,
                    from_package_name,
                    from_package_addr_name
                );
                if from_package_name == &self.source_package.package.name {
                    Some((from_package_addr_name, renaming))
                } else {
                    None
                }
            })
            .collect::<BTreeMap<_, _>>();

        println!("ADDRS: {:#?}", addrs);

        for (name, option_assignment) in self.resolution_table.iter_mut() {
            println!(
                "{} -> {:?} [{:?}",
                name,
                addrs.get(name),
                resolution_context.get(addrs.get(name).unwrap_or_else(|| &name).as_ref())
            );
            match (
                resolution_context.get(addrs.get(name).unwrap_or_else(|| &name).as_ref()),
                &option_assignment,
            ) {
                (None, _) | (Some(None), _) => {
                    println!(
                        "NO RESOLVE {} in {}",
                        name, self.source_package.package.name
                    );
                }
                (Some(Some(assigned_value)), None) => {
                    println!(
                        "Resolving {} to {} in {}",
                        name,
                        assigned_value.short_str_lossless(),
                        self.source_package.package.name
                    );
                    *option_assignment = Some(assigned_value.clone());
                }
                (Some(Some(assigned_value)), Some(already_assigned_value)) => {
                    if assigned_value != already_assigned_value {
                        bail!(
                            "Reassignment of already assigned value for {} from 0x{} to 0x{} in {}",
                            name,
                            already_assigned_value.short_str_lossless(),
                            assigned_value.short_str_lossless(),
                            self.source_package.package.name
                        );
                    }
                }
            }
        }

        Ok(())
    }

    // the resolution table contains the transitive closure of addresses that are known in that
    // package.
    fn extend_resolution_table(
        resolution_table: &mut ResolutionTable,
        dep_name: &IdentStr,
        dep_resolution_table: ResolutionTable,
    ) -> Result<()> {
        // 1. check for duplicate names in rename_to
        for (addr_name, addr_value) in dep_resolution_table.into_iter() {
            if let Some(Some(other_val)) = resolution_table.insert(addr_name.clone(), addr_value) {
                // Either it was assigned to a value and that value agrees with the previous
                // assignment, or the old value was renamed away, and is now being re-assigned.
                if Some(other_val) != addr_value {
                    bail!(
                        "Named address {} in dependency {} is already set to 0x{} but was then reassigned to {}",
                        &addr_name,
                        dep_name,
                        other_val.short_str_lossless(),
                        match addr_value {
                            None => "unassigned".to_string(),
                            Some(addr) => format!("0x{}", addr.short_str_lossless()),
                        }
                    );
                }
            }
        }

        Ok(())
    }
}
