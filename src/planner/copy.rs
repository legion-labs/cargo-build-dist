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
        // copy the binaries files
        for binary in &docker_package.binaries {
            let mut source = PathBuf::from(&docker_package.target_dir.binary_dir);
            source.push(binary);
            if !source.exists() {
                return Err(format!("file {} does'nt exist", source.display()));
            }

            let mut destination = PathBuf::from(&docker_package.target_dir.docker_dir);
            destination.push(binary);

            copy_files.push(CopyFile {
                source,
                destination,
            });
        }

        // copy the extra files
        if let Some(extra_copies) = &docker_package.docker_settings.extra_copies {
            for copy_command in extra_copies {
                let source = PathBuf::from(&copy_command.source);
                if !source.exists(){
                    return Err(format!("Error, source path {} doesn't exists", source.display()));
                }
                let mut destination = PathBuf::from(&docker_package.target_dir.docker_dir);
                if let Some(filename) = source.file_name(){
                    destination.push(filename);
                }
                
                copy_files.push(CopyFile {
                    source,
                    destination,
                });
            }
        }

        Ok(Self { copy_files })
    }
}

impl Action for CopyFiles {
    fn run(&self, verbose: bool) -> Result<(), String> {
        for copy_file in &self.copy_files {
            if verbose {
                println!("Copy file source:{}, destination:{}", &copy_file.source.display(), &copy_file.destination.display());
            }
            if let Err(e) = fs::copy(&copy_file.source, &copy_file.destination) {
                return Err(format!("failed to copy file {}", e));
            }
        }
        Ok(())
    }

    fn dryrun(&self) -> Result<(), String> {
        println!("--------------");
        println!("| Copy Files |");
        println!("--------------");
        for copy_file in &self.copy_files {
            println!(
                "Copy file Source:{} Destination:{}",
                &copy_file.source.display(),
                &copy_file.destination.display()
            );
        }
        Ok(())
    }
}
