use regex::Regex;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(
        "alias must be 1-30 chars long, and have only letters, numbers, underscores and dashes"
    )]
    InvalidAliasFormat { alias: String },
}

pub fn alias_validator(alias: &str) -> Result<String, Error> {
    let regex = Regex::new(r"^[a-zA-Z0-9_-]{1,30}$").unwrap();

    if regex.is_match(alias) {
        Ok(alias.into())
    } else {
        Err(Error::InvalidAliasFormat {
            alias: alias.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_validator_with_valid_inputs() {
        let valid_inputs = [
            "hello",
            "123",
            "hello123",
            "hello_123",
            "123_hello",
            "123-hello",
            "hello-123",
            "HeLlo-123",
        ];

        for input in valid_inputs {
            let result = alias_validator(input);
            assert!(result.is_ok());
            assert!(result.unwrap() == input);
        }
    }

    #[test]
    fn test_alias_validator_with_invalid_inputs() {
        let invalid_inputs = ["", "invalid!", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"];

        for input in invalid_inputs {
            let result = alias_validator(input);
            assert!(result.is_err());
        }
    }
}
