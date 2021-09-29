use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, convert::TryFrom, fmt::Display, path::PathBuf, str::FromStr};

#[derive(Debug, Clone)]
pub struct RustPath {
    parts: Vec<String>,
}

impl RustPath {
    pub fn head_tail(&self) -> (&str, Option<RustPath>) {
        match self.parts.len() {
            0 => panic!("Should not be able to have zero sized RustPath"),
            1 => (&self.parts[0], None),
            _ => (
                &self.parts[0],
                Some(RustPath {
                    parts: self.parts[1..].to_owned(),
                }),
            ),
        }
    }
}

impl FromStr for RustPath {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split("::").map(str::to_string).collect();
        Ok(Self { parts })
    }
}

impl Display for RustPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join("::"))
    }
}

#[derive(Debug)]
pub struct CrateRoots(HashMap<String, PathBuf>);

impl CrateRoots {
    pub fn get(&self, key: &str) -> Option<&PathBuf> {
        self.0.get(key)
    }
}

#[derive(Debug, Deserialize)]
struct CargoToml {
    package: CargoTomlPackage,
}

#[derive(Debug, Deserialize)]
struct CargoTomlPackage {
    name: String,
}

impl TryFrom<Vec<String>> for CrateRoots {
    type Error = anyhow::Error;

    fn try_from(values: Vec<String>) -> Result<Self, Self::Error> {
        let rv = values
            .iter()
            .map(|s| {
                if let Some((name, path)) = s.split_once("=") {
                    let path: PathBuf = shellexpand::tilde(path).to_string().into();
                    Ok((name.to_string(), path))
                } else {
                    let crate_path: PathBuf = shellexpand::tilde(s).to_string().into();
                    let cargo_toml_path = crate_path.join("Cargo.toml");
                    let cargo_toml_bytes = std::fs::read(&cargo_toml_path).context(format!(
                        "Reading cargo toml at {}",
                        cargo_toml_path.to_string_lossy()
                    ))?;
                    let data: CargoToml = toml::from_slice(&cargo_toml_bytes).context(format!(
                        "Parsing cargo.toml at {}",
                        cargo_toml_path.to_string_lossy()
                    ))?;

                    Ok((data.package.name, crate_path))
                }
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(Self(rv))
    }
}
