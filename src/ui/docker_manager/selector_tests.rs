//! Tests for the Docker manager selector.
//!
//! Included from `selector.rs` with `#[path]`, so they are a child module of
//! it and can still reach its private state while living in their own file.

use super::*;

#[test]
fn test_host_selection_flow() {
    let mut selector = DockerManagerSelector::new();

    // Simulate loading hosts - one local, two remote
    let ssh_hosts = vec![
        (
            1u32,
            "host1.example.com".to_string(),
            22u16,
            Some("My Server".to_string()),
            false,
        ),
        (2u32, "host2.example.com".to_string(), 22u16, None, true), // has_creds = true
    ];

    selector.load_available_hosts(&ssh_hosts);

    // Verify hosts are loaded
    assert_eq!(selector.available_hosts().len(), 3); // Local + 2 remote

    // Start host selection
    selector.start_host_selection();
    assert_eq!(selector.mode(), DockerManagerMode::HostSelection);
    assert_eq!(selector.host_selection_index(), 0);

    // Navigate to first remote host (index 1)
    selector.select_next_host();
    assert_eq!(selector.host_selection_index(), 1);

    // Check selected host display
    let host = selector.selected_host_display().unwrap();
    assert!(!host.is_local());
    assert_eq!(host.host_id, Some(1));
    assert!(!host.has_credentials); // No creds for host 1
}

#[test]
fn test_credential_flow_no_creds() {
    let mut selector = DockerManagerSelector::new();

    // Load host without credentials
    let ssh_hosts = vec![
        (1u32, "host1.example.com".to_string(), 22u16, None, false), // has_creds = false
    ];
    selector.load_available_hosts(&ssh_hosts);

    // Start host selection and navigate to remote
    selector.start_host_selection();
    selector.select_next_host(); // Move to remote host

    // Check that host has no credentials
    let host = selector.selected_host_display().unwrap();
    assert!(!host.has_credentials);

    // Start credential entry
    selector.start_host_credentials(1);

    // Verify mode changed to HostCredentials
    assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);
    assert_eq!(selector.cred_host_id(), Some(1));

    // Verify credential form is initialized
    assert_eq!(selector.cred_field(), HostCredentialField::Username);
    assert!(selector.cred_username().is_empty());
    assert!(selector.cred_password().is_empty());
}

#[test]
fn test_credential_entry() {
    let mut selector = DockerManagerSelector::new();

    // Start credential entry
    selector.start_host_credentials(1);

    // Enter username
    for c in "testuser".chars() {
        selector.cred_insert_char(c);
    }
    assert_eq!(selector.cred_username(), "testuser");

    // Move to password field
    selector.next_cred_field();
    assert_eq!(selector.cred_field(), HostCredentialField::Password);

    // Enter password
    for c in "secret123".chars() {
        selector.cred_insert_char(c);
    }
    assert_eq!(selector.cred_password(), "secret123");

    // Get entered credentials
    let (user, pass, save) = selector.get_entered_credentials();
    assert_eq!(user, "testuser");
    assert_eq!(pass, "secret123");
    assert!(!save); // Default is false
}

#[test]
fn test_host_display_is_local() {
    let local = DockerHostDisplay::local();
    assert!(local.is_local());
    assert!(local.host_id.is_none());

    let remote = DockerHostDisplay::remote(
        1,
        "example.com".to_string(),
        "user".to_string(),
        None,
        false,
    );
    assert!(!remote.is_local());
    assert_eq!(remote.host_id, Some(1));
}

/// Tests the decision logic for when to prompt for credentials.
/// This simulates the logic from docker_confirm_host_selection.
#[test]
fn test_credential_prompt_decision_logic() {
    // Scenario 1: Remote host with has_credentials=false
    // Expected: Should prompt for credentials
    let host1 = DockerHostDisplay::remote(1, "h1.com".to_string(), "u".to_string(), None, false);
    assert!(!host1.is_local());
    assert!(!host1.has_credentials);
    // In this case, code enters `else` branch and calls start_host_credentials

    // Scenario 2: Remote host with has_credentials=true but password is None
    // Expected: Should prompt for credentials (after looking up and finding no password)
    let host2 = DockerHostDisplay::remote(2, "h2.com".to_string(), "u".to_string(), None, true);
    assert!(!host2.is_local());
    assert!(host2.has_credentials);
    // In this case, code enters `if has_creds` branch, looks up creds,
    // and should prompt if password.is_none() or password.is_empty()

    // Scenario 3: Local host
    // Expected: Should NOT prompt, just select local
    let host3 = DockerHostDisplay::local();
    assert!(host3.is_local());
    // In this case, code calls docker_select_local_host and returns

    // Verify mode transitions work correctly
    let mut selector = DockerManagerSelector::new();

    // Start host selection
    selector.start_host_selection();
    assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

    // Start credential entry
    selector.start_host_credentials(1);
    assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);

    // Cancel should go back to host selection
    selector.cancel_host_credentials();
    assert_eq!(selector.mode(), DockerManagerMode::HostSelection);
}

/// Tests that password_missing logic handles all edge cases
#[test]
fn test_password_missing_logic() {
    // Simulating the password check from docker_confirm_host_selection:
    // let password_missing = password.as_ref().is_none_or(|p| p.is_empty());

    fn is_password_missing(password: &Option<String>) -> bool {
        password.as_ref().is_none_or(|p| p.is_empty())
    }

    // Case 1: None
    assert!(is_password_missing(&None));

    // Case 2: Some("")
    assert!(is_password_missing(&Some(String::new())));

    // Case 3: Some("password")
    assert!(!is_password_missing(&Some("password".to_string())));

    // Case 4: Some(" ") - whitespace only (still a password)
    assert!(!is_password_missing(&Some(" ".to_string())));
}

/// Integration test: simulates host selection with existing credentials
#[test]
fn test_full_flow_host_with_credentials() {
    let mut selector = DockerManagerSelector::new();

    // Load hosts - one has credentials, one doesn't
    let ssh_hosts = vec![
        (
            1u32,
            "host1.example.com".to_string(),
            22u16,
            Some("Host With Creds".to_string()),
            true,
        ),
        (
            2u32,
            "host2.example.com".to_string(),
            22u16,
            Some("Host Without Creds".to_string()),
            false,
        ),
    ];
    selector.load_available_hosts(&ssh_hosts);

    // Verify 3 hosts loaded (Local + 2 remote)
    assert_eq!(selector.available_hosts().len(), 3);

    // Start host selection
    selector.start_host_selection();
    assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

    // Index 0 = Local, Index 1 = Host With Creds, Index 2 = Host Without Creds
    assert_eq!(selector.host_selection_index(), 0);

    // Navigate to Host With Creds (index 1)
    selector.select_next_host();
    assert_eq!(selector.host_selection_index(), 1);

    // Get selected host
    let selected = selector.selected_host_display().unwrap();
    assert_eq!(selected.display_name, "Host With Creds");
    assert_eq!(selected.host_id, Some(1));
    assert!(selected.has_credentials); // Has saved creds

    // Since has_credentials=true, the app logic would:
    // 1. Look up SSH credentials
    // 2. Check if password exists
    // 3. If password exists -> use it, no prompt
    // 4. If password missing -> call start_host_credentials

    // Navigate to Host Without Creds (index 2)
    selector.select_next_host();
    assert_eq!(selector.host_selection_index(), 2);

    let selected = selector.selected_host_display().unwrap();
    assert_eq!(selected.display_name, "Host Without Creds");
    assert_eq!(selected.host_id, Some(2));
    assert!(!selected.has_credentials); // No saved creds

    // Since has_credentials=false, app would call start_host_credentials
    selector.start_host_credentials(2);
    assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);
    assert_eq!(selector.cred_host_id(), Some(2));
}

/// Test that mode stays correct through the flow
#[test]
fn test_mode_transitions_complete_flow() {
    let mut selector = DockerManagerSelector::new();

    // Initial mode
    assert_eq!(selector.mode(), DockerManagerMode::List);

    // Start host selection
    selector.start_host_selection();
    assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

    // Start credential entry
    selector.start_host_credentials(1);
    assert_eq!(selector.mode(), DockerManagerMode::HostCredentials);

    // Enter credentials
    for c in "testuser".chars() {
        selector.cred_insert_char(c);
    }
    selector.next_cred_field();
    for c in "testpass".chars() {
        selector.cred_insert_char(c);
    }

    // Verify credentials were entered
    let (user, pass, _) = selector.get_entered_credentials();
    assert_eq!(user, "testuser");
    assert_eq!(pass, "testpass");

    // Cancel credentials should go back to host selection
    selector.cancel_host_credentials();
    assert_eq!(selector.mode(), DockerManagerMode::HostSelection);

    // Cancel host selection should go back to list
    selector.cancel_host_selection();
    assert_eq!(selector.mode(), DockerManagerMode::List);
}
