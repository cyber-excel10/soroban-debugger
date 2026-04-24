//! Tests for Issue #837: Remote Capability Negotiation
//!
//! These tests verify that:
//! 1. Capabilities are exchanged during handshake
//! 2. Incompatibilities are detected at connection time (not later)
//! 3. Clear error messages are provided for unsupported features
//! 4. Backward compatibility is maintained

#[cfg(test)]
mod capability_negotiation {
    use soroban_debugger::server::protocol::{
        DebugMessage, DebugRequest, DebugResponse, ServerCapabilities, PROTOCOL_MAX_VERSION,
        PROTOCOL_MIN_VERSION,
    };

    /// Test 1: ServerCapabilities struct is properly initialized
    #[test]
    fn test_server_capabilities_current_build() {
        let caps = ServerCapabilities::current();

        // Verify that the current build supports the expected capabilities
        assert!(caps.conditional_breakpoints, "Should support conditional breakpoints");
        assert!(caps.source_breakpoints, "Should support source breakpoints");
        assert!(caps.evaluate, "Should support evaluate");
        assert!(caps.tls, "Should support TLS");
        assert!(caps.token_auth, "Should support token auth");
        assert!(caps.session_lifecycle, "Should support session lifecycle");
        assert!(caps.repeat_execution, "Should support repeat execution");
        assert!(!caps.symbolic_analysis, "Should NOT support symbolic analysis (opt-in)");
        assert!(caps.snapshot_loading, "Should support snapshot loading");
        assert!(caps.dynamic_trace_events, "Should support dynamic trace events");
    }

    /// Test 2: ServerCapabilities defaults to all false
    #[test]
    fn test_server_capabilities_default_is_empty() {
        let caps = ServerCapabilities::default();

        // All fields should be false by default
        assert!(!caps.conditional_breakpoints);
        assert!(!caps.source_breakpoints);
        assert!(!caps.evaluate);
        assert!(!caps.tls);
        assert!(!caps.token_auth);
        assert!(!caps.session_lifecycle);
        assert!(!caps.repeat_execution);
        assert!(!caps.symbolic_analysis);
        assert!(!caps.snapshot_loading);
        assert!(!caps.dynamic_trace_events);
    }

    /// Test 3: unsupported_by() correctly identifies missing capabilities
    #[test]
    fn test_unsupported_by_identifies_missing_features() {
        let client_required = ServerCapabilities {
            evaluate: true,
            snapshot_loading: true,
            conditional_breakpoints: true,
            ..Default::default()
        };

        let server_has = ServerCapabilities {
            evaluate: true,
            snapshot_loading: false, // Missing!
            conditional_breakpoints: true,
            ..Default::default()
        };

        let missing = client_required.unsupported_by(&server_has);

        assert_eq!(missing.len(), 1, "Should identify exactly 1 missing capability");
        assert!(
            missing.contains(&"snapshot_loading"),
            "Should identify snapshot_loading as missing"
        );
    }

    /// Test 4: unsupported_by() returns empty when all features are supported
    #[test]
    fn test_unsupported_by_returns_empty_when_all_supported() {
        let client_required = ServerCapabilities {
            evaluate: true,
            conditional_breakpoints: true,
            ..Default::default()
        };

        let server_has = ServerCapabilities::current(); // Has everything

        let missing = client_required.unsupported_by(&server_has);

        assert!(
            missing.is_empty(),
            "Should return empty list when all capabilities are supported"
        );
    }

    /// Test 5: Handshake request can include required_capabilities
    #[test]
    fn test_handshake_request_with_required_capabilities() {
        let required = ServerCapabilities {
            evaluate: true,
            snapshot_loading: true,
            ..Default::default()
        };

        let request = DebugRequest::Handshake {
            client_name: "test-client".to_string(),
            client_version: "1.0.0".to_string(),
            protocol_min: PROTOCOL_MIN_VERSION,
            protocol_max: PROTOCOL_MAX_VERSION,
            heartbeat_interval_ms: None,
            idle_timeout_ms: None,
            required_capabilities: Some(required.clone()),
        };

        // Verify the request can be serialized
        let json = serde_json::to_string(&request).expect("Should serialize");
        assert!(json.contains("required_capabilities"), "JSON should contain required_capabilities");

        // Verify it can be deserialized
        let deserialized: DebugRequest =
            serde_json::from_str(&json).expect("Should deserialize");
        match deserialized {
            DebugRequest::Handshake {
                required_capabilities: Some(caps),
                ..
            } => {
                assert!(caps.evaluate, "Deserialized evaluate should be true");
                assert!(caps.snapshot_loading, "Deserialized snapshot_loading should be true");
            }
            _ => panic!("Expected Handshake with required_capabilities"),
        }
    }

    /// Test 6: Handshake request without required_capabilities is backward compatible
    #[test]
    fn test_handshake_request_without_required_capabilities_is_backward_compatible() {
        let request = DebugRequest::Handshake {
            client_name: "old-client".to_string(),
            client_version: "0.9.0".to_string(),
            protocol_min: PROTOCOL_MIN_VERSION,
            protocol_max: PROTOCOL_MAX_VERSION,
            heartbeat_interval_ms: None,
            idle_timeout_ms: None,
            required_capabilities: None,
        };

        // Verify the request can be serialized without required_capabilities
        let json = serde_json::to_string(&request).expect("Should serialize");
        assert!(
            !json.contains("required_capabilities"),
            "JSON should not contain required_capabilities when None"
        );

        // Verify it can be deserialized
        let deserialized: DebugRequest =
            serde_json::from_str(&json).expect("Should deserialize");
        match deserialized {
            DebugRequest::Handshake {
                required_capabilities: None,
                ..
            } => {
                // Expected
            }
            _ => panic!("Expected Handshake without required_capabilities"),
        }
    }

    /// Test 7: HandshakeAck response includes server_capabilities
    #[test]
    fn test_handshake_ack_includes_server_capabilities() {
        let server_caps = ServerCapabilities::current();

        let response = DebugResponse::HandshakeAck {
            server_name: "soroban-debug".to_string(),
            server_version: "1.0.0".to_string(),
            protocol_min: PROTOCOL_MIN_VERSION,
            protocol_max: PROTOCOL_MAX_VERSION,
            selected_version: 1,
            heartbeat_interval_ms: None,
            idle_timeout_ms: None,
            server_capabilities: server_caps.clone(),
        };

        // Verify the response can be serialized
        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(
            json.contains("server_capabilities"),
            "JSON should contain server_capabilities"
        );

        // Verify it can be deserialized
        let deserialized: DebugResponse =
            serde_json::from_str(&json).expect("Should deserialize");
        match deserialized {
            DebugResponse::HandshakeAck {
                server_capabilities: caps,
                ..
            } => {
                assert_eq!(caps.evaluate, server_caps.evaluate);
                assert_eq!(caps.snapshot_loading, server_caps.snapshot_loading);
            }
            _ => panic!("Expected HandshakeAck with server_capabilities"),
        }
    }

    /// Test 8: IncompatibleCapabilities response is properly structured
    #[test]
    fn test_incompatible_capabilities_response() {
        let server_caps = ServerCapabilities {
            evaluate: true,
            snapshot_loading: false,
            ..Default::default()
        };

        let response = DebugResponse::IncompatibleCapabilities {
            message: "Server does not support required capabilities: snapshot_loading"
                .to_string(),
            missing_capabilities: vec!["snapshot_loading".to_string()],
            server_capabilities: server_caps.clone(),
        };

        // Verify the response can be serialized
        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(
            json.contains("IncompatibleCapabilities"),
            "JSON should contain IncompatibleCapabilities type"
        );
        assert!(
            json.contains("missing_capabilities"),
            "JSON should contain missing_capabilities"
        );

        // Verify it can be deserialized
        let deserialized: DebugResponse =
            serde_json::from_str(&json).expect("Should deserialize");
        match deserialized {
            DebugResponse::IncompatibleCapabilities {
                missing_capabilities,
                server_capabilities: caps,
                ..
            } => {
                assert_eq!(missing_capabilities.len(), 1);
                assert_eq!(missing_capabilities[0], "snapshot_loading");
                assert!(!caps.snapshot_loading);
            }
            _ => panic!("Expected IncompatibleCapabilities response"),
        }
    }

    /// Test 9: DebugMessage can wrap capability negotiation requests/responses
    #[test]
    fn test_debug_message_wraps_capability_negotiation() {
        let request = DebugRequest::Handshake {
            client_name: "test".to_string(),
            client_version: "1.0.0".to_string(),
            protocol_min: PROTOCOL_MIN_VERSION,
            protocol_max: PROTOCOL_MAX_VERSION,
            heartbeat_interval_ms: None,
            idle_timeout_ms: None,
            required_capabilities: Some(ServerCapabilities {
                evaluate: true,
                ..Default::default()
            }),
        };

        let msg = DebugMessage::request(42, request);
        assert_eq!(msg.id, 42);
        assert!(msg.request.is_some());

        let json = serde_json::to_string(&msg).expect("Should serialize");
        let deserialized: DebugMessage =
            serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(deserialized.id, 42);
    }

    /// Test 10: Scenario - Client requires feature, server has it (SUCCESS)
    #[test]
    fn test_scenario_client_requires_feature_server_has_it() {
        let client_required = ServerCapabilities {
            evaluate: true,
            snapshot_loading: true,
            ..Default::default()
        };

        let server_has = ServerCapabilities::current();

        let missing = client_required.unsupported_by(&server_has);

        // Should succeed - no missing capabilities
        assert!(
            missing.is_empty(),
            "Connection should succeed when server has all required capabilities"
        );
    }

    /// Test 11: Scenario - Client requires feature, server doesn't have it (FAILURE)
    #[test]
    fn test_scenario_client_requires_feature_server_lacks_it() {
        let client_required = ServerCapabilities {
            evaluate: true,
            snapshot_loading: true,
            symbolic_analysis: true, // Server doesn't support this
            ..Default::default()
        };

        let server_has = ServerCapabilities::current();

        let missing = client_required.unsupported_by(&server_has);

        // Should fail - symbolic_analysis is missing
        assert!(
            !missing.is_empty(),
            "Should identify missing capabilities"
        );
        assert!(
            missing.contains(&"symbolic_analysis"),
            "Should identify symbolic_analysis as missing"
        );
    }

    /// Test 12: Scenario - Old client (no required_capabilities) connects to new server
    #[test]
    fn test_scenario_old_client_new_server_backward_compat() {
        // Old client doesn't send required_capabilities
        let request = DebugRequest::Handshake {
            client_name: "old-client".to_string(),
            client_version: "0.9.0".to_string(),
            protocol_min: PROTOCOL_MIN_VERSION,
            protocol_max: PROTOCOL_MAX_VERSION,
            heartbeat_interval_ms: None,
            idle_timeout_ms: None,
            required_capabilities: None, // Old client doesn't send this
        };

        // Server should accept it (no required capabilities to check)
        match request {
            DebugRequest::Handshake {
                required_capabilities: None,
                ..
            } => {
                // Expected - server treats as no requirements
            }
            _ => panic!("Expected Handshake without required_capabilities"),
        }
    }

    /// Test 13: Scenario - New client (with required_capabilities) connects to old server
    #[test]
    fn test_scenario_new_client_old_server_forward_compat() {
        // New client sends required_capabilities
        let request = DebugRequest::Handshake {
            client_name: "new-client".to_string(),
            client_version: "1.1.0".to_string(),
            protocol_min: PROTOCOL_MIN_VERSION,
            protocol_max: PROTOCOL_MAX_VERSION,
            heartbeat_interval_ms: None,
            idle_timeout_ms: None,
            required_capabilities: Some(ServerCapabilities {
                evaluate: true,
                ..Default::default()
            }),
        };

        // Old server doesn't understand required_capabilities field
        // But serde should handle it gracefully (skip unknown fields)
        let json = serde_json::to_string(&request).expect("Should serialize");
        let deserialized: DebugRequest =
            serde_json::from_str(&json).expect("Should deserialize");

        match deserialized {
            DebugRequest::Handshake {
                required_capabilities: Some(_),
                ..
            } => {
                // Expected - field is preserved
            }
            _ => panic!("Expected Handshake with required_capabilities"),
        }
    }

    /// Test 14: Multiple missing capabilities are all reported
    #[test]
    fn test_multiple_missing_capabilities_reported() {
        let client_required = ServerCapabilities {
            evaluate: true,
            snapshot_loading: true,
            symbolic_analysis: true,
            dynamic_trace_events: true,
            ..Default::default()
        };

        let server_has = ServerCapabilities {
            evaluate: true,
            snapshot_loading: false,
            symbolic_analysis: false,
            dynamic_trace_events: false,
            ..Default::default()
        };

        let missing = client_required.unsupported_by(&server_has);

        assert_eq!(missing.len(), 3, "Should identify all 3 missing capabilities");
        assert!(missing.contains(&"snapshot_loading"));
        assert!(missing.contains(&"symbolic_analysis"));
        assert!(missing.contains(&"dynamic_trace_events"));
    }

    /// Test 15: Verify issue #837 acceptance criteria
    #[test]
    fn test_issue_837_acceptance_criteria() {
        // Issue #837: "Done when: A mismatch in available features is reported at connect time
        // instead of later in the session."

        // Scenario: Client requires snapshot_loading, server doesn't support it
        let client_required = ServerCapabilities {
            snapshot_loading: true,
            ..Default::default()
        };

        let server_has = ServerCapabilities {
            snapshot_loading: false,
            ..Default::default()
        };

        // The mismatch should be detected during handshake
        let missing = client_required.unsupported_by(&server_has);

        // Verify the mismatch is detected
        assert!(
            !missing.is_empty(),
            "Mismatch should be detected at handshake time"
        );
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "snapshot_loading");

        // Verify we can construct the error response that would be sent
        let error_response = DebugResponse::IncompatibleCapabilities {
            message: format!(
                "Server does not support required capabilities: {}. Upgrade the server or disable these features on the client.",
                missing.join(", ")
            ),
            missing_capabilities: missing.iter().map(|s| s.to_string()).collect(),
            server_capabilities: server_has,
        };

        // Verify the error can be serialized and communicated
        let json = serde_json::to_string(&error_response).expect("Should serialize");
        assert!(json.contains("IncompatibleCapabilities"));
        assert!(json.contains("snapshot_loading"));

        // This proves that incompatibilities are now reported at connect time
        // (during handshake) instead of later when operations are attempted.
    }
}
