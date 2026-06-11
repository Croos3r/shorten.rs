use std::{sync::LazyLock, time::Duration};

use regex::Regex;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

static RE_HTTP_SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https?://.+").unwrap());
static RE_PASSWORD_UPPERCASE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Z]").unwrap());
static RE_PASSWORD_SPECIAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[#?!@$%^&*-]").unwrap());
static RE_PASSWORD_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]").unwrap());

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExpirationOptions {
    #[default]
    Hour,
    Day,
    Week,
    Never,
    Custom(Duration),
}

impl From<ExpirationOptions> for Duration {
    fn from(value: ExpirationOptions) -> Self {
        Duration::from_hours(match value {
            ExpirationOptions::Hour => 1,
            ExpirationOptions::Day => 24,
            ExpirationOptions::Week => 7 * 24,
            ExpirationOptions::Never => return Duration::MAX,
            ExpirationOptions::Custom(duration) => return duration,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Validate)]
pub struct ShortenUrlDto {
    #[validate(url, regex(path = *RE_HTTP_SCHEME))]
    pub(crate) url: String,
    pub(crate) expire_in: Option<ExpirationOptions>,
}

fn validate_password(password: &str) -> Result<(), ValidationError> {
    if RE_PASSWORD_NUMBER.find(password).is_none() {
        return Err(ValidationError::new("no_number"));
    }

    if RE_PASSWORD_SPECIAL.find(password).is_none() {
        return Err(ValidationError::new("no_special"));
    }

    if RE_PASSWORD_UPPERCASE.find(password).is_none() {
        return Err(ValidationError::new("no_uppercase"));
    }

    Ok(())
}

#[derive(Debug, Deserialize, Serialize, Clone, Validate)]
pub struct RegisterDto {
    #[validate(length(min = 2, max = 255))]
    pub(crate) name: String,
    #[validate(length(max = 255), email)]
    pub(crate) email: String,
    #[validate(
        custom(function = "validate_password"),
        must_match(other = "confirmed_password"),
        length(min = 8)
    )]
    pub(crate) password: String,
    #[validate(must_match(other = "password"))]
    pub(crate) confirmed_password: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Validate)]
pub struct LoginDto {
    #[validate(email, length(max = 255))]
    pub(crate) email: String,
    pub(crate) password: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiration_option_defaults_to_one_hour() {
        assert!(matches!(
            ExpirationOptions::default(),
            ExpirationOptions::Hour
        ));
        assert_eq!(
            Duration::from(ExpirationOptions::default()),
            Duration::from_hours(1)
        );
    }

    #[test]
    fn named_expiration_options_convert_to_their_durations() {
        assert_eq!(
            Duration::from(ExpirationOptions::Hour),
            Duration::from_hours(1)
        );
        assert_eq!(
            Duration::from(ExpirationOptions::Day),
            Duration::from_hours(24)
        );
        assert_eq!(
            Duration::from(ExpirationOptions::Week),
            Duration::from_hours(7 * 24)
        );
    }

    #[test]
    fn never_converts_to_the_maximum_duration() {
        // `Never` maps to the largest representable duration rather than a fixed
        // far-future date, so the conversion itself must be the saturating one.
        assert_eq!(Duration::from(ExpirationOptions::Never), Duration::MAX);
    }

    #[test]
    fn custom_expiration_passes_its_duration_through_unchanged() {
        let ttl = Duration::from_secs(1234);
        assert_eq!(Duration::from(ExpirationOptions::Custom(ttl)), ttl);
    }

    #[test]
    fn named_expiration_options_deserialize_from_their_lowercase_names() {
        // `#[serde(rename_all = "lowercase")]` is what lets a query string carry
        // `expire_in=week`; lock that wire format in.
        assert!(matches!(
            serde_json::from_str::<ExpirationOptions>("\"hour\"").unwrap(),
            ExpirationOptions::Hour
        ));
        assert!(matches!(
            serde_json::from_str::<ExpirationOptions>("\"week\"").unwrap(),
            ExpirationOptions::Week
        ));
        assert!(matches!(
            serde_json::from_str::<ExpirationOptions>("\"never\"").unwrap(),
            ExpirationOptions::Never
        ));
    }

    #[test]
    fn shorten_url_dto_treats_a_missing_expiration_as_none() {
        let dto: ShortenUrlDto = serde_json::from_str(r#"{"url":"https://example.com"}"#).unwrap();
        assert!(dto.expire_in.is_none());
    }
}
