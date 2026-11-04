use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Hls,
    Dl,
}

impl Scope {
    fn prefix(self) -> &'static str {
        match self {
            Scope::Hls => "hls",
            Scope::Dl => "dl",
        }
    }
}

fn mac(secret: &[u8], scope: Scope, id: &str, exp: u64) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(format!("{}|{id}|{exp}", scope.prefix()).as_bytes());
    mac
}

pub fn sign(secret: &[u8], scope: Scope, id: &str, exp: u64) -> String {
    hex::encode(mac(secret, scope, id, exp).finalize().into_bytes())
}

pub fn verify(secret: &[u8], scope: Scope, id: &str, exp: u64, sig_hex: &str, now: u64) -> bool {
    if now > exp {
        return false;
    }
    let Ok(sig) = hex::decode(sig_hex) else {
        return false;
    };
    mac(secret, scope, id, exp).verify_slice(&sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-secret";
    const ID: &str = "0123456789abcdef0123456789abcdef";
    const EXP: u64 = 1767225600;
    const HLS_VECTOR: &str = "398897b4462277ed24b8ba6d55289edfaa2e8a8ed79cad3a28502649b6a375ec";
    const DL_VECTOR: &str = "997950fe99c0e0f162c61a580b7b51e2eb1e992c3e9ee924e6a90e5a4807de88";

    #[test]
    fn vector_hls() {
        assert_eq!(sign(SECRET, Scope::Hls, ID, EXP), HLS_VECTOR);
    }

    #[test]
    fn scopes_differ() {
        let dl = sign(SECRET, Scope::Dl, ID, EXP);
        assert_ne!(dl, HLS_VECTOR);
        assert_eq!(dl, DL_VECTOR);
        assert!(verify(SECRET, Scope::Dl, ID, EXP, &dl, EXP));
        assert!(!verify(SECRET, Scope::Hls, ID, EXP, &dl, EXP));
    }

    #[test]
    fn expiry_boundary() {
        assert!(verify(SECRET, Scope::Hls, ID, EXP, HLS_VECTOR, EXP));
        assert!(!verify(SECRET, Scope::Hls, ID, EXP, HLS_VECTOR, EXP + 1));
    }

    #[test]
    fn rejects_garbage() {
        let mut flipped = HLS_VECTOR.to_string();
        flipped.replace_range(0..1, "4");
        for bad in ["zz", "", flipped.as_str()] {
            assert!(!verify(SECRET, Scope::Hls, ID, EXP, bad, EXP));
        }
    }
}
