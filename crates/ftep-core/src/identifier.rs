use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;
use std::str::FromStr;

const MAX_IDENTIFIER_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierError {
    value: String,
    reason: &'static str,
}

impl IdentifierError {
    fn new(value: impl Into<String>, reason: &'static str) -> Self {
        Self {
            value: value.into(),
            reason,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid identifier {:?}: {}",
            self.value, self.reason
        )
    }
}

impl std::error::Error for IdentifierError {}

fn validate(value: &str) -> Result<(), IdentifierError> {
    if value.is_empty() {
        return Err(IdentifierError::new(value, "must not be empty"));
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(IdentifierError::new(
            value,
            "must contain at most 64 ASCII bytes",
        ));
    }
    if !value.is_ascii() {
        return Err(IdentifierError::new(value, "must contain only ASCII"));
    }
    if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
        return Err(IdentifierError::new(
            value,
            "hyphens must separate non-empty segments",
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(IdentifierError::new(
            value,
            "use lowercase ASCII letters, digits, and single hyphens",
        ));
    }
    Ok(())
}

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                validate(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl FromStr for $name {
            type Err = IdentifierError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(de::Error::custom)
            }
        }
    };
}

identifier!(GameId);
identifier!(GameVariantId);
identifier!(RuntimeId);
identifier!(FranchiseId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_stable_slug_identifiers() {
        assert_eq!(GameId::new("zelda-oot").unwrap().as_str(), "zelda-oot");
        assert_eq!(
            RuntimeId::new("switch-runtime").unwrap().as_str(),
            "switch-runtime"
        );
    }

    #[test]
    fn rejects_ambiguous_or_unstable_identifiers() {
        for invalid in [
            "",
            "Zelda-OoT",
            "zelda_oot",
            "-zelda",
            "zelda-",
            "zelda--oot",
            "zeldá",
        ] {
            assert!(GameId::new(invalid).is_err(), "{invalid:?} should fail");
        }
    }

    #[test]
    fn deserialization_cannot_bypass_validation() {
        let error = serde_json::from_str::<RuntimeId>(r#""Shipwright""#).unwrap_err();
        assert!(error.to_string().contains("lowercase ASCII"));
    }
}
