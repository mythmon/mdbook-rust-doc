use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, convert::TryFrom, fmt::Display, path::PathBuf, str::FromStr};

#[derive(Debug, Clone, PartialEq)]
pub struct RustPath {
    head: String,
    tail: Option<Vec<String>>,
}

impl RustPath {
    #[must_use]
    pub fn head_tail(&self) -> (&str, Option<Self>) {
        match &self.tail {
            None => (self.head.as_str(), None),
            Some(vec) if vec.is_empty() => (self.head.as_str(), None),
            Some(vec) if vec.len() == 1 => (
                self.head.as_str(),
                Some(Self {
                    head: vec[0].to_string(),
                    tail: None,
                }),
            ),
            Some(vec) => (
                self.head.as_str(),
                Some(Self {
                    head: vec[0].to_string(),
                    tail: Some(vec[1..].to_owned()),
                }),
            ),
        }
    }
}

impl FromStr for RustPath {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.split("::").map(str::to_string).collect();
        match parts.len() {
            0 => Err(anyhow!("Zero length RustPaths are not allowed")),
            1 => Ok(Self {
                head: s.to_owned(),
                tail: None,
            }),
            _ => Ok(Self {
                head: parts[0].to_string(),
                tail: Some(parts[1..].to_owned()),
            }),
        }
    }
}

impl Display for RustPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.head)?;
        if let Some(tail) = &self.tail {
            write!(f, "::{}", tail.join("::"))?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CrateRoots(HashMap<String, PathBuf>);

impl CrateRoots {
    #[must_use]
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

#[cfg(test)]
mod tests {
    use crate::RustPath;
    use std::str::FromStr;

    #[test]
    fn test_single() {
        assert_eq!(
            RustPath::from_str("one").unwrap(),
            RustPath {
                head: "one".to_string(),
                tail: None
            }
        );
    }

    #[test]
    fn test_double() {
        assert_eq!(
            RustPath::from_str("one::two").unwrap(),
            RustPath {
                head: "one".to_string(),
                tail: Some(vec!["two".to_string()])
            }
        );
    }

    #[test]
    fn test_triple() {
        assert_eq!(
            RustPath::from_str("one::two::three").unwrap(),
            RustPath {
                head: "one".to_string(),
                tail: Some(vec!["two".to_string(), "three".to_string()])
            }
        );
    }

    #[test]
    fn test_tuple_indexes() {
        assert_eq!(
            RustPath::from_str("a_tuple::0").unwrap(),
            RustPath {
                head: "a_tuple".to_string(),
                tail: Some(vec!["0".to_string()])
            }
        );
    }

    #[test]
    fn test_failure_1() {
        assert_eq!(
            RustPath::from_str("test_crate::crustaceans::CookedCrab::0").unwrap(),
            RustPath {
                head: "test_crate".to_string(),
                tail: Some(vec![
                    "crustaceans".to_string(),
                    "CookedCrab".to_string(),
                    "0".to_string(),
                ])
            }
        );
    }
}
