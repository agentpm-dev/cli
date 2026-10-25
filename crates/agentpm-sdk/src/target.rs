use std::{fmt, str::FromStr};

include!(concat!(env!("OUT_DIR"), "/targets.rs"));

impl Target {
    pub fn current() -> anyhow::Result<Self> {
        env!("AGENTPM_BUILD_TARGET").parse()
    }

    pub fn current_triple() -> &'static str {
        env!("AGENTPM_BUILD_TARGET")
    }
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "any" {
            return Ok(Self::Any);
        }
        Self::SUPPORTED
            .iter()
            .copied()
            .find(|t| t.as_str() == value)
            .ok_or_else(|| anyhow::anyhow!("unsupported AgentPM target: {value}"))
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for Target {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Target {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_vocabulary_and_build_mapping() {
        for target in Target::SUPPORTED.iter().chain([Target::Any].iter()) {
            assert_eq!(target.as_str().parse::<Target>().unwrap(), *target);
        }
        assert!("x86_64-unknown-linux-musl".parse::<Target>().is_err());
        assert!("aarch64-unknown-linux-gnu".parse::<Target>().is_err());
        assert_eq!(
            Target::current().unwrap().as_str(),
            Target::current_triple()
        );
    }
}
