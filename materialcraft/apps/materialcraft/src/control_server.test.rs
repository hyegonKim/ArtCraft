#[cfg(test)]
mod auth_tests {
    use crate::control_server::auth_ok;
    use serde_json::json;

    #[test]
    fn only_the_exact_token_on_an_auth_request_passes() {
        let token = "0123456789abcdef0123456789abcdef";
        assert!(auth_ok(token, "auth", &json!({"token": token})));
        assert!(!auth_ok(token, "auth", &json!({"token": "0123456789abcdef0123456789abcdee"})));
        assert!(!auth_ok(token, "auth", &json!({"token": "short"})));
        assert!(!auth_ok(token, "auth", &json!({})));
        assert!(!auth_ok(token, "auth", &json!({"token": 5})));
        assert!(!auth_ok(token, "command.run", &json!({"token": token})));
    }
}
