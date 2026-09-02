use std::path::PathBuf;

pub struct Paths {
    pub modula: PathBuf,
}

impl Paths {
    pub fn from_env() -> anyhow::Result<Self> {
        let modula = modula_platform::modula_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
        Ok(Self { modula })
    }
}
