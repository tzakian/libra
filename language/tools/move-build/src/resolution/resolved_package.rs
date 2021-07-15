//use crate::source_package::{
//    layout::SourcePackageLayout,
//    manifest_parser::{parse_move_manifest_string, parse_source_manifest},
//    parsed_manifest::{Dependency, SourceManifest, SubstOrRename},
//    resolution::resolution_graph::ResolutionGraph,
//};
//use anyhow::Result;
//use move_core_types::{
//    identifier::Identifier,
//    account_address::AccountAddress,
//};
//use petgraph::{graph::NodeIndex, Directed, Graph};
//use std::{
//    collections::BTreeMap,
//    path::{Path, PathBuf},
//};
//
//#[derive(Debug, Clone)]
//pub struct ResolvedPackageContext {
//    pub root_package: NodeIndex<u32>,
//    pub dependency_graph: Graph<Identifier, Identifier, Directed>,
//    pub packages: BTreeMap<Identifier, ResolvedPackage>,
//    pub substitution: BTreeMap<Identifier, AccountAddress>,
//}
//
//#[derive(Debug, Clone)]
//pub struct ResolvedPackage {
//    pub graph_index: NodeIndex<u32>,
//    pub source_manifest: SourceManifest,
//    pub package_path: PathBuf,
//    pub renamings: BTreeMap<Identifier, Identifier>,
//}
//
//
//impl ResolvedPackageContext {
//    pub fn new(resolution_graph: ResolutionGraph) -> Result<ResolvedPackageContext> {
//        let root_node_id = resolution_graph.memo_table[&resolution_graph.root_package.name];
//        let mut substitution = BTreeMap::new();
//        let mut packages = BTreeMap::new();
//        for (name, package) in resolution_graph.memo_table.into_iter() {
//            let (resolved_package, resolved_subst) = ResolvedPackage::new(package)?;
//            packages.insert(name, resolved_package);
//            Self::unify(&mut substitution, resolved_subst)?;
//            Self::resolve_to_substitution(resolution_graph.resolution_table)?;
//        }
//
//        Ok(ResolvedPackageContext {
//            root_package: root_node_id,
//            dependency_graph: resolution_graph.graph,
//            packages,
//            substitution,
//        })
//
//        std::todo!()
//
//    }
//
//    fn resolve_to_substitution(resolution_table: BTreeMap<Identifier, SubstOrRename>) -> Result<BTreeMap<Identifier, AccountAddress>> {
//
//        resolution_table.into_iter().map
//
//    }
//}
//
//impl ResolvedPackage {
//
//    pub fn new(node: ResolutionNode) -> Result<(ResolvedPackage, BTreeMap<Identifier, AccountAddress>)> {
//        std::todo!()
//    }
//}
