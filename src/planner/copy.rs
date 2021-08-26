use std::{fs, path::PathBuf};

use crate::{Action, DockerPackage};

pub struct CopyFile {
    source: PathBuf,
    destination: PathBuf,
}

pub struct CopyFiles {
    copy_files: Vec<CopyFile>,
}

impl CopyFiles {
    pub fn new(docker_package: &DockerPackage) -> Result<Self, String> {
        let mut copy_files = vec![];
        for binary in &docker_package.binaries {
            let mut source = PathBuf::new();
            source.push(&docker_package.target_dir.binary_dir);
            source.push(binary);

            if !source.exists() {
                return Err(format!("file {} does'nt exist", source.display()));
            }

            let mut destination = PathBuf::new();
            destination.push(&docker_package.target_dir.docker_dir);
            destination.push(binary);

            copy_files.push(CopyFile {
                source,
                destination,
            });
        }
        Ok(Self { copy_files })
    }
}

impl Action for CopyFiles {
    fn run(&self) -> Result<(), String> {
        for copy_file in &self.copy_files {
            if let Err(e) = fs::copy(&copy_file.source, &copy_file.destination) {
                return Err(format!("failed to copy file {}", e));
            }
        }
        Ok(())
    }
    fn dryrun(&self) -> Result<(), String> {
        for copy_file in &self.copy_files {
            println!(
                "Copy file from {} to {}",
                &copy_file.source.display(),
                &copy_file.destination.display()
            );
        }
        Ok(())
    }
}
