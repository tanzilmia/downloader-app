use crate::process::{args, failure, run};
use download_core::{AppError, ErrorCode, ToolInfo};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolSpec {
    pub path: PathBuf,
    pub sha256: String,
    pub version: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolManifest {
    pub schema_version: u32,
    pub target: String,
    pub distribution: String,
    pub tools: BTreeMap<String, ToolSpec>,
}

#[derive(Clone)]
pub struct Toolchain {
    pub tools: BTreeMap<String, ToolSpec>,
}

impl Toolchain {
    pub async fn load(manifest_path: &Path) -> Result<Self, AppError> {
        if tokio::fs::metadata(manifest_path)
            .await
            .map_err(|_| {
                failure(
                    ErrorCode::Dependency,
                    "The local media engine is not installed. Run the engine setup command.",
                    false,
                )
            })?
            .len()
            > 65536
        {
            return Err(failure(
                ErrorCode::Dependency,
                "The media tool manifest is too large.",
                false,
            ));
        }
        let bytes = tokio::fs::read(manifest_path).await.map_err(|_| failure(ErrorCode::Dependency, "The local media engine is not installed. Install a verified engine package, then restart Downpour.", false))?;
        if bytes.len() > 65536 {
            return Err(failure(
                ErrorCode::Dependency,
                "The engine manifest is too large.",
                false,
            ));
        }
        let mut manifest: ToolManifest = serde_json::from_slice(&bytes).map_err(|_| {
            failure(
                ErrorCode::Dependency,
                "The engine manifest is invalid. Repair the engine installation.",
                false,
            )
        })?;
        if manifest.schema_version != 1
            || manifest.target != host_target()
            || !["development", "portable"].contains(&manifest.distribution.as_str())
        {
            return Err(failure(
                ErrorCode::Dependency,
                "This engine package is incompatible with the current platform.",
                false,
            ));
        }
        let directory = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let base = tokio::fs::canonicalize(directory)
            .await
            .map_err(AppError::storage)?;
        for name in ["yt-dlp", "ffmpeg", "ffprobe", "deno"] {
            #[cfg(debug_assertions)]
            eprintln!("Downpour: verifying {name}");
            let tool = manifest.tools.get_mut(name).ok_or_else(|| {
                failure(
                    ErrorCode::Dependency,
                    "The engine package is missing a required tool.",
                    false,
                )
            })?;
            if manifest.distribution == "portable" {
                if tool.path.is_absolute()
                    || tool
                        .path
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                {
                    return Err(failure(
                        ErrorCode::Dependency,
                        "Portable engine paths must stay inside their package.",
                        false,
                    ));
                }
                tool.path = directory.join(&tool.path);
            } else if !tool.path.is_absolute() {
                return Err(failure(
                    ErrorCode::Dependency,
                    "Development tool paths must be absolute.",
                    false,
                ));
            }
            tool.path = tokio::fs::canonicalize(&tool.path).await.map_err(|_| {
                failure(
                    ErrorCode::Dependency,
                    "A media executable is missing. Repair the engine installation.",
                    false,
                )
            })?;
            if manifest.distribution == "portable" && !tool.path.starts_with(&base) {
                return Err(failure(
                    ErrorCode::Dependency,
                    "A portable media executable escaped its package.",
                    false,
                ));
            }
            verify(tool).await?;
        }
        Ok(Self {
            tools: manifest.tools,
        })
    }
    pub fn tool(&self, name: &str) -> &ToolSpec {
        self.tools.get(name).expect("validated tool name")
    }
    pub fn info(&self) -> Vec<ToolInfo> {
        self.tools
            .iter()
            .map(|(name, tool)| ToolInfo {
                name: name.clone(),
                version: tool.version.clone(),
            })
            .collect()
    }
    pub async fn check_versions(&self, cwd: &Path) -> Result<(), AppError> {
        for name in ["yt-dlp", "ffmpeg", "ffprobe", "deno"] {
            #[cfg(debug_assertions)]
            eprintln!("Downpour: checking {name} version");
            let tool = self.tool(name);
            verify(tool).await?;
            let output = run(
                &tool.path,
                &args(&[if name == "ffmpeg" || name == "ffprobe" {
                    "-version"
                } else {
                    "--version"
                }]),
                cwd,
                Duration::from_secs(if name == "yt-dlp" { 45 } else { 15 }),
                CancellationToken::new(),
                None,
            )
            .await?;
            if !output.success {
                return Err(failure(
                    ErrorCode::Dependency,
                    "A verified media executable failed its startup check.",
                    false,
                ));
            }
        }
        Ok(())
    }
}

pub async fn sha256(path: &Path, cancel: &CancellationToken) -> Result<(u64, String), AppError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(AppError::storage)?;
    let mut hash = Sha256::new();
    let mut count = 0;
    // Keep the hashing buffer off nested async initialization stacks.
    let mut bytes = vec![0_u8; 65536];
    loop {
        if cancel.is_cancelled() {
            return Err(failure(ErrorCode::Interrupted, "Transfer cancelled.", true));
        }
        let read = file.read(&mut bytes).await.map_err(AppError::storage)?;
        if read == 0 {
            break;
        }
        hash.update(&bytes[..read]);
        count += read as u64;
    }
    Ok((count, hex::encode(hash.finalize())))
}
pub async fn verify(tool: &ToolSpec) -> Result<(), AppError> {
    let metadata = tokio::fs::metadata(&tool.path).await.map_err(|_| {
        failure(
            ErrorCode::Dependency,
            "A media executable is missing.",
            false,
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 || tool.sha256.len() != 64 {
        return Err(failure(
            ErrorCode::Dependency,
            "A media executable or its checksum is invalid.",
            false,
        ));
    }
    let (_, hash) = sha256(&tool.path, &CancellationToken::new()).await?;
    if hash != tool.sha256 {
        return Err(failure(ErrorCode::Dependency, "A media executable changed after installation. Repair the engine package before continuing.", false));
    }
    Ok(())
}
pub fn host_target() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else {
        "unsupported"
    }
}
