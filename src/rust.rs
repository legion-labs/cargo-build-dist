use std::process::Command;

use log::debug;

use crate::{Error, ErrorContext, Result};

pub fn is_current_target_runtime(target_runtime: &str) -> Result<bool> {
    let current_target_runtime = get_current_target_runtime()?;
    if target_runtime == current_target_runtime {
        debug!(
            "Current target runtime `{}` matches desired target runtime",
            target_runtime
        );
        Ok(true)
    } else {
        debug!(
            "Current target runtime `{}` does not match desired target runtime `{}`",
            current_target_runtime, target_runtime
        );
        Ok(false)
    }
}

pub fn get_current_target_runtime() -> Result<String> {
    let output = Command::new("rustc")
        .args(["--print", "cfg"])
        .output()
        .map_err(|err| {
            Error::new("failed to determine current Rust runtime target").with_source(err)
        })?
        .stdout;

    let output = String::from_utf8(output).unwrap();

    let mut arch = None;
    let mut vendor = None;
    let mut os = None;
    let mut env = None;

    for line in output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "target_arch" => {
                    arch = Some(unquote(value).with_context("failed to unquote target_arch")?)
                }
                "target_vendor" => {
                    vendor = Some(unquote(value).with_context("failed to unquote target_vendor")?)
                }
                "target_os" => {
                    os = Some(unquote(value).with_context("failed to unquote target_os")?)
                }
                "target_env" => {
                    env = Some(unquote(value).with_context("failed to unquote target_env")?)
                }
                _ => (),
            }
        }
    }

    match (arch, vendor, os, env) {
        (Some(arch), Some(vendor), Some(os), Some(env)) => {
            let mut target = arch.to_string();

            target.push_str("-");
            target.push_str(vendor);
            target.push_str("-");
            target.push_str(os);
            target.push_str("-");
            target.push_str(env);

            Ok(target)
        }
        _ => Err(Error::new(
            "failed to determine current Rust runtime target",
        )),
    }
}

fn unquote(s: &str) -> Result<&str> {
    if s.starts_with('"') && s.ends_with('"') {
        Ok(&s[1..s.len() - 1])
    } else {
        Err(Error::new("failed to unquote string")
            .with_output(format!("s: {}", s))
            .with_explanation("The string was supposed to be quoted but it wasn't."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_target_runtime() {
        assert!(get_current_target_runtime().is_ok());
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote("\"foo\"").unwrap(), "foo");
        assert_eq!(unquote("\"f o o\"").unwrap(), "f o o");

        unquote("\"foo").unwrap_err();
        unquote("foo\"").unwrap_err();
        unquote("foo").unwrap_err();
    }
}
