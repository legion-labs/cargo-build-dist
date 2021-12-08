use std::{cmp::Ordering, ffi::OsStr, path::Path, process::Command};

use itertools::Itertools;

use crate::{action_step, metadata::Metadata, sources::Sources, Context, Error, Result};

/// A package in the workspace.
#[derive(Debug, Clone)]
pub struct Package<'g> {
    context: &'g Context,
    package_metadata: guppy::graph::PackageMetadata<'g>,
    monorepo_metadata: Metadata,
    sources: Sources,
}

impl<'g> Package<'g> {
    pub(crate) fn new(
        context: &'g Context,
        package_metadata: guppy::graph::PackageMetadata<'g>,
    ) -> Result<Self> {
        let monorepo_metadata = Metadata::new(&package_metadata)?;
        let sources = Sources::from_package(context, &package_metadata)?;

        Ok(Self {
            context,
            package_metadata,
            monorepo_metadata,
            sources,
        })
    }

    pub fn id(&self) -> &guppy::PackageId {
        self.package_metadata.id()
    }

    pub fn name(&self) -> &str {
        self.package_metadata.name()
    }

    pub fn version(&self) -> &semver::Version {
        self.package_metadata.version()
    }

    pub fn directly_dependant_packages(&self) -> Result<Vec<Package<'g>>> {
        self.package_metadata
            .reverse_direct_links()
            .map(|package_link| Package::new(self.context, package_link.from()))
            .collect()
    }

    pub fn dependant_packages(&self) -> Result<Vec<Package<'g>>> {
        self.directly_dependant_packages()?
            .into_iter()
            .map(|package| {
                package
                    .directly_dependant_packages()
                    .map(|packages| std::iter::once(package).chain(packages.into_iter()))
            })
            .collect::<Result<Vec<_>>>()
            .map(|packages| packages.into_iter().flatten().collect())
    }

    pub fn sources(&self) -> &Sources {
        &self.sources
    }

    pub fn root(&self) -> &Path {
        self.package_metadata
            .manifest_path()
            .parent()
            .unwrap()
            .as_std_path()
    }

    pub fn build_dist_targets(&self) -> Result<()> {
        unimplemented!()
    }

    pub fn publish_dist_targets(&self) -> Result<()> {
        unimplemented!()
    }

    pub fn tag(&self) -> Result<()> {
        unimplemented!()
    }

    pub fn execute(
        &self,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Result<std::process::ExitStatus> {
        let args: Vec<_> = args.into_iter().collect();

        if args.is_empty() {
            return Err(Error::new("no arguments provided to execute"));
        }

        action_step!("Executing", "{}", self.package_metadata.id());
        action_step!(
            "Running",
            "`{}`",
            args.iter().map(|s| s.as_ref().to_string_lossy()).join(" "),
        );

        let program = args[0].as_ref();
        let program_args = &args[1..];
        let mut cmd = Command::new(program);

        cmd.args(program_args)
            .current_dir(&self.package_metadata.manifest_path().parent().unwrap());

        cmd.status()
            .map_err(|err| Error::new("failed to execute command").with_source(err))
    }

    pub fn hash(&self) -> String {
        "hash".to_string()
    }

    ///// Check that the current tag matches the current hash.
    //pub fn tag_matches(&self, context: &Context) -> Result<bool> {
    //    let tags = self.tags(context)?;
    //    let version = self.version();
    //    let hash = self.hash();

    //    if let Some(current_hash) = tags.versions.get(version) {
    //        return Ok(current_hash == &hash);
    //    }

    //    Ok(false)
    //}

    ///// Tag the package with its current version and hash.
    /////
    ///// If a tag already exist for the version, the call will fail.
    //pub fn tag(&self, options: &Options) -> Result<()> {
    //    let version = self.version();
    //    let hash = self.hash();

    //    let tags_file = Self::tags_file(&self.package);
    //    let mut tags = Tags::read_file(&tags_file)?;

    //    if let Some(current_hash) = tags.versions.get(version) {
    //        if current_hash == &hash {
    //            ignore_step!(
    //                "Skipping",
    //                "tagging {} as a tag with an identical hash `{}` exists already",
    //                self.id(),
    //                hash,
    //            );

    //            return Ok(());
    //        }

    //        if options.force {
    //            action_step!("Re-tagging", "{} with hash `{}`", self.id(), &hash);
    //            Ok(())
    //        } else {
    //            Err(Error::new("tag already exists for version")
    //                .with_explanation(format!(
    //                    "A tag for version `{}` already exists with a different hash `{}`. You may need to increment the package version number and try again.",
    //                    version,
    //                    current_hash,
    //                ))
    //            )
    //        }
    //    } else {
    //        action_step!("Tagging", "{} with hash `{}`", self.id(), &hash);

    //        Ok(())
    //    }?;

    //    tags.versions.insert(version.clone(), hash);
    //    tags.write_file(&tags_file)
    //}
}

impl Eq for Package<'_> {}

impl Ord for Package<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id().cmp(&other.id())
    }
}

impl PartialOrd for Package<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Package<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
