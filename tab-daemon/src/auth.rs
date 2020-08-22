use rand::{rngs::OsRng, RngCore};

pub fn gen_token() -> String {
    let mut token = vec![0; 128];
    OsRng.fill_bytes(token.as_mut_slice());

    base64::encode(token.as_slice())
}
