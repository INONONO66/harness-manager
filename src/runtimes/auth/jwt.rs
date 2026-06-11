pub(super) use crate::runtimes::types::AuthStatus;

pub(super) fn token_to_auth_status(token: &str, label: &str) -> AuthStatus {
    let expiry = decode_jwt_expiry(token);
    match expiry {
        Some(JwtExpiry::Expired) => AuthStatus::Expired {
            detail: format!("{} (expired)", label),
        },
        Some(JwtExpiry::ExpiresSoon(exp)) => AuthStatus::ExpiresSoon {
            detail: format!("{} ({})", label, exp),
        },
        Some(JwtExpiry::Valid(exp)) => AuthStatus::Valid {
            detail: format!("{} ({})", label, exp),
        },
        None => AuthStatus::Valid {
            detail: label.to_string(),
        },
    }
}

enum JwtExpiry {
    Valid(String),
    ExpiresSoon(String),
    Expired,
}

fn decode_jwt_expiry(token: &str) -> Option<JwtExpiry> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{}==", payload),
        3 => format!("{}=", payload),
        _ => payload.to_string(),
    };
    let decoded_str = padded.replace('-', "+").replace('_', "/");
    let bytes = base64_decode(&decoded_str)?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;

    let exp = json.get("exp")?.as_u64()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if exp < now {
        return Some(JwtExpiry::Expired);
    }

    let remaining = exp - now;
    let days = remaining / 86400;
    let hours = (remaining % 86400) / 3600;

    let detail = if days > 0 {
        format!("{}d {}h left", days, hours)
    } else if hours > 0 {
        format!("{}h left", hours)
    } else {
        format!("{}m left", (remaining % 3600) / 60)
    };

    if remaining < 86_400 {
        Some(JwtExpiry::ExpiresSoon(detail))
    } else {
        Some(JwtExpiry::Valid(detail))
    }
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &b in input.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = TABLE.iter().position(|&c| c == b)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(output)
}

#[cfg(test)]
mod jwt_expiry_tests {
    use super::{base64_decode, token_to_auth_status, AuthStatus};

    #[test]
    fn jwt_expiring_within_one_hour_returns_expires_soon() {
        // Given: a JWT access token that expires soon.
        let token = jwt_with_exp_delta(600);

        // When: hm classifies the auth token.
        let status = token_to_auth_status(&token, "OAuth");

        // Then: detect can render it as a warning instead of a healthy check.
        match status {
            AuthStatus::ExpiresSoon { detail } => {
                assert!(
                    detail.contains("OAuth"),
                    "label should be retained: {detail}"
                );
                assert!(
                    detail.contains("left"),
                    "expiry detail should be retained: {detail}"
                );
            }
            other => panic!("expected ExpiresSoon, got {other:?}"),
        }
    }

    #[test]
    fn base64_decode_handles_padded_payload() {
        assert_eq!(base64_decode("eyJ4IjoxfQ=="), Some(br#"{"x":1}"#.to_vec()));
    }

    #[test]
    fn base64_decode_rejects_non_base64_input() {
        assert!(base64_decode("not valid!").is_none());
    }

    fn jwt_with_exp_delta(delta_secs: u64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let payload = format!(r#"{{"exp":{}}}"#, now + delta_secs);
        format!(
            "{}.{}.{}",
            base64_url(r#"{"alg":"none"}"#),
            base64_url(&payload),
            "sig"
        )
    }

    fn base64_url(input: &str) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = input.as_bytes();
        let mut output = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = *chunk.get(1).unwrap_or(&0);
            let b2 = *chunk.get(2).unwrap_or(&0);
            output.push(TABLE[(b0 >> 2) as usize] as char);
            output.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                output.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
            }
            if chunk.len() > 2 {
                output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
            }
        }
        output.replace('+', "-").replace('/', "_")
    }
}
