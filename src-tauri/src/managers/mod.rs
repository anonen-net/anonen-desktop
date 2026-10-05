pub mod anonen_cloud_auth;
pub mod audio;
pub mod engine_slot;
pub mod history;
pub mod model;
pub mod transcription;
pub mod usage;

use log::warn;

pub fn force_https(url: &str) -> String {
    let trimmed = url.trim();
    let Some(rest) = trimmed.strip_prefix("http://") else {
        return trimmed.to_string();
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if is_loopback(authority) {
        return trimmed.to_string();
    }
    warn!("[anonen-cloud] endpoint is http://; using https instead (cleartext is not allowed)");
    format!("https://{}", rest)
}

pub fn build_time_setting(baked: Option<&str>, var: &str) -> Option<String> {
    let Some(baked) = baked.map(str::trim).filter(|v| !v.is_empty()) else {
        return std::env::var(var).ok();
    };
    if std::env::var(var).is_ok_and(|from_env| from_env.trim() != baked) {
        warn!("[anonen-cloud] ignoring {var} from the environment: this build has its own value");
    }
    Some(baked.to_string())
}

pub fn reads_dotenv_at_runtime(debug_build: bool, baked_endpoint: Option<&str>) -> bool {
    debug_build || baked_endpoint.is_none_or(|url| url.trim().is_empty())
}

fn is_loopback(authority: &str) -> bool {
    let host = match authority.strip_prefix('[') {
        Some(rest) => match rest.split_once(']') {
            Some((host, _)) => host,
            None => return false,
        },
        None => authority.split(':').next().unwrap_or(""),
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_endpoints_are_left_alone() {
        assert_eq!(
            "https://api.anonen.net",
            force_https("https://api.anonen.net")
        );
        assert_eq!(
            "https://your-project.supabase.co",
            force_https("  https://your-project.supabase.co  ")
        );
    }

    #[test]
    fn a_cleartext_endpoint_is_upgraded_rather_than_trusted() {
        assert_eq!(
            "https://api.anonen.net",
            force_https("http://api.anonen.net")
        );
        assert_eq!(
            "https://api.anonen.net/v1",
            force_https("http://api.anonen.net/v1")
        );
    }

    #[test]
    fn a_local_gateway_may_still_be_plain_http() {
        for url in [
            "http://localhost:8000",
            "http://127.0.0.1:8000",
            "http://127.0.0.1",
            "http://[::1]:8000",
            "http://[::1]",
            "http://LOCALHOST:8000",
        ] {
            assert_eq!(url, force_https(url), "{url}");
        }
    }

    #[test]
    fn a_hostname_that_merely_starts_with_localhost_is_not_loopback() {
        assert_eq!(
            "https://localhost.example.com/v1",
            force_https("http://localhost.example.com/v1")
        );
        assert_eq!(
            "https://127.0.0.1.example.com",
            force_https("http://127.0.0.1.example.com")
        );
    }

    #[test]
    fn a_shipped_build_cannot_be_pointed_at_another_gateway() {
        std::env::set_var("ANONEN_TEST_CLOUD_URL", "https://someone-elses.example");
        assert_eq!(
            Some("https://api.anonen.net"),
            build_time_setting(Some("https://api.anonen.net"), "ANONEN_TEST_CLOUD_URL").as_deref()
        );
    }

    #[test]
    fn the_environment_cannot_reopen_the_plaintext_route_on_a_shipped_build() {
        std::env::set_var("ANONEN_TEST_SEALED_REQUIRED", "0");
        assert_eq!(
            Some("true"),
            build_time_setting(Some("true"), "ANONEN_TEST_SEALED_REQUIRED").as_deref()
        );
    }

    #[test]
    fn a_build_with_nothing_baked_in_still_reads_the_environment() {
        std::env::set_var("ANONEN_TEST_DEV_ONLY", "http://localhost:8000");
        assert_eq!(
            Some("http://localhost:8000"),
            build_time_setting(None, "ANONEN_TEST_DEV_ONLY").as_deref()
        );
    }

    #[test]
    fn a_baked_value_that_is_empty_does_not_pin_anything() {
        std::env::set_var("ANONEN_TEST_EMPTY_BAKED", "https://staging.example");
        assert_eq!(
            Some("https://staging.example"),
            build_time_setting(Some("   "), "ANONEN_TEST_EMPTY_BAKED").as_deref()
        );
    }

    #[test]
    fn a_shipped_build_does_not_read_a_dotenv_next_to_the_exe() {
        assert!(!reads_dotenv_at_runtime(
            false,
            Some("https://api.anonen.net")
        ));
    }

    #[test]
    fn a_development_build_still_reads_its_dotenv() {
        assert!(reads_dotenv_at_runtime(
            true,
            Some("https://api.anonen.net")
        ));
    }

    #[test]
    fn a_build_with_no_endpoint_baked_in_has_to_read_one_from_somewhere() {
        assert!(reads_dotenv_at_runtime(false, None));
        assert!(reads_dotenv_at_runtime(false, Some("  ")));
    }

    #[test]
    fn nothing_baked_and_nothing_in_the_environment_is_none() {
        assert_eq!(
            None,
            build_time_setting(None, "ANONEN_TEST_NEVER_SET_ANYWHERE")
        );
    }
}
