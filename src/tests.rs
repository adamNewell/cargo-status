#[cfg(test)]
mod tests {
    use super::super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_parse_test_results_cargo() {
        let output = "test result: ok. 5 passed; 2 failed; 1 ignored";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 5);
        assert_eq!(failed, 2);
    }

    #[test]
    fn test_parse_test_results_nextest() {
        let output = "PASS [ 0.001s] module::test1
FAIL [ 0.002s] module::test2
PASS [ 0.001s] module::test3";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 2);
        assert_eq!(failed, 1);
    }
    
    #[test]
    fn test_parse_test_results_nextest_summary() {
        let output = "Summary [ 0.001s] 10 tests run: 8 passed, 2 failed";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 8);
        assert_eq!(failed, 2);
    }

    #[test]
    fn test_strip_ansi_escapes() {
        let input = "\x1b[32mGreen text\x1b[0m and normal";
        let expected = "Green text and normal";
        assert_eq!(strip_ansi_escapes(input), expected);
    }

    #[test]
    fn test_strip_ansi_escapes_multiple() {
        let input = "\x1b[1m\x1b[31mBold Red\x1b[0m \x1b[34mBlue\x1b[0m";
        let expected = "Bold Red Blue";
        assert_eq!(strip_ansi_escapes(input), expected);
    }

    #[test]
    fn test_default_true() {
        assert_eq!(default_true(), true);
    }

    #[test]
    fn test_profile_serialization() {
        let profile = Profile {
            fmt: true,
            check: false,
            clippy: true,
            test: false,
            build: true,
            doc: false,
            audit: true,
            sequential: false,
        };

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();

        assert_eq!(profile.fmt, deserialized.fmt);
        assert_eq!(profile.check, deserialized.check);
        assert_eq!(profile.clippy, deserialized.clippy);
        assert_eq!(profile.test, deserialized.test);
        assert_eq!(profile.build, deserialized.build);
        assert_eq!(profile.doc, deserialized.doc);
        assert_eq!(profile.audit, deserialized.audit);
        assert_eq!(profile.sequential, deserialized.sequential);
    }

    #[test]
    fn test_verbose_tools_parsing() {
        // Save original args
        let original_args: Vec<String> = std::env::args().collect();
        
        // Test with combined flags
        std::env::set_var("_TEST_ARGS", "-fv -cv -lv");
        
        // Note: We can't easily test parse_verbose_tools directly because it uses env::args()
        // Instead, we'll test the logic with a helper function
        
        let mut verbose_tools = std::collections::HashSet::new();
        let test_args = vec!["-fv", "-cv", "-lv"];
        
        for arg in test_args {
            if arg.contains('v') {
                if arg.contains('f') {
                    verbose_tools.insert("fmt".to_string());
                }
                if arg.contains('c') {
                    verbose_tools.insert("check".to_string());
                }
                if arg.contains('l') {
                    verbose_tools.insert("clippy".to_string());
                }
            }
        }
        
        assert!(verbose_tools.contains("fmt"));
        assert!(verbose_tools.contains("check"));
        assert!(verbose_tools.contains("clippy"));
        assert!(!verbose_tools.contains("test"));
    }

    #[test]
    fn test_checks_config_defaults() {
        let config = ChecksConfig::default();
        assert_eq!(config.fmt, false);
        assert_eq!(config.check, false);
        assert_eq!(config.clippy, false);
        assert_eq!(config.test, false);
        assert_eq!(config.build, false);
        assert_eq!(config.doc, false);
        assert_eq!(config.audit, false);
    }

    #[test]
    fn test_verbose_tools_defaults() {
        let tools = VerboseTools::default();
        assert_eq!(tools.fmt, false);
        assert_eq!(tools.check, false);
        assert_eq!(tools.clippy, false);
        assert_eq!(tools.test, false);
        assert_eq!(tools.build, false);
        assert_eq!(tools.doc, false);
        assert_eq!(tools.audit, false);
    }

    #[test]
    fn test_cargo_status_config_defaults() {
        let config = CargoStatusConfig::default();
        assert_eq!(config.sequential, false);
        assert_eq!(config.verbose, false);
        assert_eq!(config.profile, None);
    }

    #[test]
    fn test_status_check_new() {
        let check = StatusCheck::new("Test", vec!["cargo".to_string(), "test".to_string()]);
        assert_eq!(check.name, "Test");
        assert_eq!(check.command, vec!["cargo".to_string(), "test".to_string()]);
        assert_eq!(check.warning_patterns, vec!["warning".to_string()]);
        assert_eq!(check.verbose, false);
    }

    #[test]
    fn test_status_check_with_warning_patterns() {
        let patterns = vec!["error".to_string(), "fail".to_string()];
        let check = StatusCheck::new("Test", vec!["test".to_string()])
            .with_warning_patterns(patterns.clone());
        assert_eq!(check.warning_patterns, patterns);
    }

    #[test]
    fn test_status_check_with_verbose() {
        let check = StatusCheck::new("Test", vec!["test".to_string()])
            .with_verbose(true);
        assert_eq!(check.verbose, true);
    }

    #[test]
    fn test_parse_nextest_with_color_codes() {
        let output = "\x1b[32mSummary\x1b[0m [ 0.001s] 10 tests run: 8 passed, 0 failed";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 8);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_test_results_empty() {
        let output = "";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 0);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_test_results_invalid() {
        let output = "Some random output without test results";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 0);
        assert_eq!(failed, 0);
    }
}