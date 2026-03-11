use serde::Serialize;

#[derive(Serialize)]
struct SuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    ok: bool,
    error: String,
}

pub fn success<T: Serialize>(data: &T) -> anyhow::Result<()> {
    let envelope = SuccessEnvelope { ok: true, data };
    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
}

pub fn error_json(msg: &str) -> anyhow::Result<()> {
    let envelope = ErrorEnvelope {
        ok: false,
        error: msg.to_string(),
    };
    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
}
